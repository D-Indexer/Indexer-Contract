use super::*;

const CIDV0: &[u8] = b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
const CIDV1: &[u8] = b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";

#[test]
fn folder_name_accepts_valid_labels() {
    assert!(is_valid_folder_name(b"bob.folder"));
    assert!(is_valid_folder_name(b"a-b-c.folder"));
    assert!(is_valid_folder_name(b"user123.folder"));
}

#[test]
fn folder_name_rejects_invalid_labels() {
    assert!(!is_valid_folder_name(b"ab.folder"));
    assert!(!is_valid_folder_name(b"alice"));
    assert!(!is_valid_folder_name(b"Bad_Name.folder"));
    assert!(!is_valid_folder_name(b"-bob.folder"));
    assert!(!is_valid_folder_name(b"bob-.folder"));
    assert!(!is_valid_folder_name(b"bob.folder "));
    assert!(!is_valid_folder_name(
        b"this-folder-name-is-intentionally-longer-than-sixty-four-characters.folder"
    ));
}

#[test]
fn cid_accepts_v0_and_v1() {
    assert!(is_valid_cid(CIDV0));
    assert!(is_valid_cid(CIDV1));
}

#[test]
fn cid_rejects_malformed_values() {
    assert!(!is_valid_cid(b""));
    assert!(!is_valid_cid(b"Qm"));
    assert!(!is_valid_cid(b"QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbd0"));
    assert!(!is_valid_cid(b"bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdI"));
    assert!(!is_valid_cid(b"zdj7WkRPAX9o9nb9zPbXzwG7JEs78uyhwbUs8JSUayB98DWWY"));
}

#[test]
fn metadata_uri_accepts_cid_and_path() {
    assert!(is_valid_metadata_uri(b"ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"));
    assert!(is_valid_metadata_uri(
        b"ipfs://bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi/1.json"
    ));
}

#[test]
fn metadata_uri_rejects_bad_scheme_cid_or_path() {
    assert!(!is_valid_metadata_uri(b"https://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"));
    assert!(!is_valid_metadata_uri(b"ipfs://not-a-cid"));
    assert!(!is_valid_metadata_uri(
        b"ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG/a b"
    ));
}
