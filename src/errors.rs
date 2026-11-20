use soroban_sdk::contracterror;

// Every variant carries a doc comment: a description line, then a
// `User message:` line. `scripts/errors_json.py` turns them into errors.json.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ContractError {
    /// The contract has already been initialized.
    /// User message: This contract is already set up.
    AlreadyInitialized = 1,
    /// The contract has not been initialized yet.
    /// User message: This contract is not set up yet. Please try again later.
    NotInitialized = 2,
    /// The Folder name is already owned by another address.
    /// User message: That Folder name is already taken.
    FolderNameTaken = 3,
    /// No Folder exists for the given name or owner.
    /// User message: We could not find that Folder.
    FolderNotFound = 4,
    /// The owner has no minted portfolio.
    /// User message: No portfolio has been minted for this Folder yet.
    PortfolioNotFound = 5,
    /// The template ID is not registered.
    /// User message: That template does not exist.
    TemplateNotFound = 6,
    /// The caller is not allowed to perform this action.
    /// User message: You are not allowed to do that.
    Unauthorized = 7,
    /// The IPFS CID is malformed.
    /// User message: The portfolio content ID is not valid.
    InvalidCID = 8,
    /// The Folder name does not follow the naming rules.
    /// User message: Folder names must look like name.folder and use only a-z, 0-9 and hyphens.
    InvalidFolderName = 9,
    /// The address is already an admin.
    /// User message: That address is already an admin.
    AdminAlreadyExists = 10,
    /// The address is not an admin.
    /// User message: That address is not an admin.
    AdminNotFound = 11,
    /// Removing the admin would leave the contract without admins.
    /// User message: The last admin cannot be removed.
    CannotRemoveLastAdmin = 12,
    /// The owner has already minted a portfolio.
    /// User message: A portfolio has already been minted for this Folder.
    PortfolioAlreadyMinted = 13,
    /// The template metadata URI is malformed.
    /// User message: The template metadata link is not valid.
    InvalidMetadataURI = 14,
    /// The template ID is already registered.
    /// User message: A template with that ID already exists.
    TemplateAlreadyExists = 15,
    /// The owner already holds a Folder.
    /// User message: You already own a Folder.
    FolderAlreadyClaimed = 16,
    /// The template ID is out of range.
    /// User message: That template ID is not valid.
    InvalidTemplateID = 17,
    /// The maximum number of templates is registered.
    /// User message: No more templates can be registered.
    TemplateLimitReached = 18,
    /// The maximum number of admins is reached.
    /// User message: No more admins can be added.
    TooManyAdmins = 19,
    /// The token amount is negative, zero where not allowed, or above its cap.
    /// User message: The amount is not valid.
    InvalidAmount = 20,
    /// The contract treasury holds less than the requested amount.
    /// User message: The treasury does not hold enough funds for this withdrawal.
    InsufficientBalance = 21,
}
