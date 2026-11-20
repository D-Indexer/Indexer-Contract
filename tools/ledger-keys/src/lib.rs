//! Builds the XDR `LedgerKey` for each contract storage entry, so the backend
//! can read storage directly with RPC `getLedgerEntries`.

use indexer_contract::DataKey;
use soroban_sdk::xdr::{
    ContractDataDurability, LedgerKey, LedgerKeyContractData, Limits, ScAddress, ScVal, WriteXdr,
};
use soroban_sdk::{Address, Env, IntoVal, String, TryFromVal, Val};

/// Parsed CLI request for one storage entry.
pub enum Entry {
    /// Contract instance entry; holds `AdminList`, `Initialized` and `Treasury`.
    Instance,
    FolderOwner(std::string::String),
    OwnerFolder(std::string::String),
    Template(u32),
    TemplateList,
    TemplatePrice(u32),
    UserTemplate(std::string::String),
    Portfolio(std::string::String),
}

pub const USAGE: &str = "usage: ledger-keys <contract-id> <entry> [arg]

entries:
  instance                 contract instance (AdminList, Initialized, Treasury)
  folder-owner <name>      FolderOwner(name)
  owner-folder <address>   OwnerFolder(address)
  template <id>            Template(id)
  template-list            TemplateList
  template-price <id>      TemplatePrice(id)
  user-template <address>  UserTemplate(address)
  portfolio <address>      Portfolio(address)";

impl Entry {
    pub fn parse(kind: &str, arg: Option<&str>) -> Result<Entry, std::string::String> {
        let text = || {
            arg.map(str::to_owned)
                .ok_or(format!("{kind} needs an argument"))
        };
        let id = || {
            text()?
                .parse::<u32>()
                .map_err(|e| format!("invalid template id: {e}"))
        };
        Ok(match kind {
            "instance" | "admin-list" | "initialized" | "treasury" => Entry::Instance,
            "folder-owner" => Entry::FolderOwner(text()?),
            "owner-folder" => Entry::OwnerFolder(text()?),
            "template" => Entry::Template(id()?),
            "template-list" => Entry::TemplateList,
            "template-price" => Entry::TemplatePrice(id()?),
            "user-template" => Entry::UserTemplate(text()?),
            "portfolio" => Entry::Portfolio(text()?),
            _ => return Err(format!("unknown entry `{kind}`")),
        })
    }

    fn data_key(&self, env: &Env) -> Option<DataKey> {
        let addr = |s: &str| Address::from_str(env, s);
        Some(match self {
            Entry::Instance => return None,
            Entry::FolderOwner(name) => DataKey::FolderOwner(String::from_str(env, name)),
            Entry::OwnerFolder(a) => DataKey::OwnerFolder(addr(a)),
            Entry::Template(id) => DataKey::Template(*id),
            Entry::TemplateList => DataKey::TemplateList,
            Entry::TemplatePrice(id) => DataKey::TemplatePrice(*id),
            Entry::UserTemplate(a) => DataKey::UserTemplate(addr(a)),
            Entry::Portfolio(a) => DataKey::Portfolio(addr(a)),
        })
    }
}

/// Returns the `LedgerKey` for `entry` under `contract`. User entries live in
/// persistent storage; instance-stored values share the instance entry.
pub fn ledger_key(env: &Env, contract: &Address, entry: &Entry) -> LedgerKey {
    let key = match entry.data_key(env) {
        Some(data_key) => {
            let val: Val = data_key.into_val(env);
            ScVal::try_from_val(env, &val).expect("DataKey converts to ScVal")
        }
        None => ScVal::LedgerKeyContractInstance,
    };
    LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::from(contract),
        key,
        durability: ContractDataDurability::Persistent,
    })
}

/// Base64 XDR of the `LedgerKey`, as accepted by `getLedgerEntries`.
pub fn ledger_key_base64(contract_id: &str, entry: &Entry) -> std::string::String {
    let env = Env::default();
    let contract = Address::from_str(&env, contract_id);
    ledger_key(&env, &contract, entry)
        .to_xdr_base64(Limits::none())
        .expect("LedgerKey encodes")
}
