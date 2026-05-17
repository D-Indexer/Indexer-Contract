use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    FolderNameTaken = 3,
    FolderNotFound = 4,
    PortfolioNotFound = 5,
    TemplateNotFound = 6,
    Unauthorized = 7,
    InvalidCID = 8,
    InvalidFolderName = 9,
    AdminAlreadyExists = 10,
    AdminNotFound = 11,
    CannotRemoveLastAdmin = 12,
    PortfolioAlreadyMinted = 13,
}
