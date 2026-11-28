//! Rebuilds contract state from decoded events, following `docs/EVENTS.md`.

use std::collections::BTreeMap;

/// A decoded contract event: topic symbol plus positional data fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub ledger: u32,
    pub topic: String,
    pub data: Vec<String>,
}

/// Rebuilt state, keyed the same way the contract stores it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub admins: Vec<String>,
    pub folder_owner: BTreeMap<String, String>,
    pub owner_folder: BTreeMap<String, String>,
    pub templates: BTreeMap<u32, (String, String)>,
    pub user_template: BTreeMap<String, u32>,
    pub portfolios: BTreeMap<String, (u32, String)>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one event. Unknown topics and trailing fields are ignored so
    /// appended fields stay compatible.
    pub fn apply(&mut self, event: &Event) -> Result<(), String> {
        let field = |i: usize| -> Result<&str, String> {
            event
                .data
                .get(i)
                .map(|s| s.as_str())
                .ok_or_else(|| format!("{}: missing data field {}", event.topic, i))
        };
        match event.topic.as_str() {
            "init" => {
                let admin = field(0)?.to_string();
                if !self.admins.contains(&admin) {
                    self.admins.push(admin);
                }
            }
            "adm_add" => {
                let new_admin = field(1)?.to_string();
                if !self.admins.contains(&new_admin) {
                    self.admins.push(new_admin);
                }
            }
            "adm_rm" => {
                let target = field(1)?;
                self.admins.retain(|a| a != target);
            }
            "folder_cl" => {
                let owner = field(0)?.to_string();
                let folder = field(1)?.to_string();
                self.folder_owner.insert(folder.clone(), owner.clone());
                self.owner_folder.insert(owner, folder);
            }
            "tmpl_reg" => {
                let id = parse_u32(field(0)?)?;
                let uri = field(1)?.to_string();
                let creator = field(2)?.to_string();
                self.templates.insert(id, (uri, creator));
            }
            "tmpl_set" => {
                let owner = field(0)?.to_string();
                let id = parse_u32(field(1)?)?;
                self.user_template.insert(owner.clone(), id);
                if let Some(portfolio) = self.portfolios.get_mut(&owner) {
                    portfolio.0 = id;
                }
            }
            "port_mint" => {
                let owner = field(0)?.to_string();
                let id = parse_u32(field(1)?)?;
                let cid = field(2)?.to_string();
                self.portfolios.insert(owner.clone(), (id, cid));
                self.user_template.insert(owner, id);
            }
            "port_upd" => {
                let owner = field(0)?.to_string();
                let cid = field(1)?.to_string();
                match self.portfolios.get_mut(&owner) {
                    Some(portfolio) => portfolio.1 = cid,
                    None => return Err(format!("port_upd for unminted owner {}", owner)),
                }
            }
            other => return Err(format!("unknown event topic {}", other)),
        }
        Ok(())
    }

    /// Applies events in ledger order.
    pub fn replay(events: &[Event]) -> Result<Self, String> {
        let mut sorted = events.to_vec();
        sorted.sort_by_key(|e| e.ledger);
        let mut state = Self::new();
        for event in &sorted {
            state.apply(event)?;
        }
        Ok(state)
    }
}

fn parse_u32(raw: &str) -> Result<u32, String> {
    raw.parse::<u32>().map_err(|_| format!("invalid u32 {:?}", raw))
}

/// One difference between rebuilt and live state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diff {
    pub key: String,
    pub rebuilt: Option<String>,
    pub live: Option<String>,
}

/// Compares rebuilt state against live reads. Empty result means no diffs.
pub fn diff(rebuilt: &State, live: &State) -> Vec<Diff> {
    let mut diffs = Vec::new();
    let mut admins_rebuilt = rebuilt.admins.clone();
    let mut admins_live = live.admins.clone();
    admins_rebuilt.sort();
    admins_live.sort();
    if admins_rebuilt != admins_live {
        diffs.push(Diff {
            key: "admins".to_string(),
            rebuilt: Some(admins_rebuilt.join(",")),
            live: Some(admins_live.join(",")),
        });
    }
    diff_map(&mut diffs, "folder_owner", &rebuilt.folder_owner, &live.folder_owner);
    diff_map(&mut diffs, "owner_folder", &rebuilt.owner_folder, &live.owner_folder);
    diff_map(
        &mut diffs,
        "user_template",
        &rebuilt.user_template,
        &live.user_template,
    );
    let templates_rebuilt: BTreeMap<u32, String> = rebuilt
        .templates
        .iter()
        .map(|(k, (uri, creator))| (*k, format!("{}|{}", uri, creator)))
        .collect();
    let templates_live: BTreeMap<u32, String> = live
        .templates
        .iter()
        .map(|(k, (uri, creator))| (*k, format!("{}|{}", uri, creator)))
        .collect();
    diff_map(&mut diffs, "template", &templates_rebuilt, &templates_live);
    let portfolios_rebuilt: BTreeMap<String, String> = rebuilt
        .portfolios
        .iter()
        .map(|(k, (id, cid))| (k.clone(), format!("{}|{}", id, cid)))
        .collect();
    let portfolios_live: BTreeMap<String, String> = live
        .portfolios
        .iter()
        .map(|(k, (id, cid))| (k.clone(), format!("{}|{}", id, cid)))
        .collect();
    diff_map(&mut diffs, "portfolio", &portfolios_rebuilt, &portfolios_live);
    diffs
}

fn diff_map<K: Ord + std::fmt::Display, V: PartialEq + std::fmt::Display>(
    diffs: &mut Vec<Diff>,
    prefix: &str,
    rebuilt: &BTreeMap<K, V>,
    live: &BTreeMap<K, V>,
) {
    let mut keys: Vec<&K> = rebuilt.keys().chain(live.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let r = rebuilt.get(key);
        let l = live.get(key);
        if r != l {
            diffs.push(Diff {
                key: format!("{}:{}", prefix, key),
                rebuilt: r.map(|v| v.to_string()),
                live: l.map(|v| v.to_string()),
            });
        }
    }
}
