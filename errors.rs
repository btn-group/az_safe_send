use ink::{
    env::Error as InkEnvError,
    prelude::{format, string::String},
    LangError,
};
use openbrush::contracts::psp22::PSP22Error;

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AzSafeSendError {
    ContractCall(LangError),
    IncorrectFee,
    InkEnvError(String),
    NotFound(String),
    PSP22Error(PSP22Error),
    RecordsLimitReached(String),
    UnprocessableEntity(String),
}
impl From<InkEnvError> for AzSafeSendError {
    fn from(e: InkEnvError) -> Self {
        AzSafeSendError::InkEnvError(format!("{e:?}"))
    }
}
impl From<LangError> for AzSafeSendError {
    fn from(e: LangError) -> Self {
        AzSafeSendError::ContractCall(e)
    }
}
impl From<PSP22Error> for AzSafeSendError {
    fn from(e: PSP22Error) -> Self {
        AzSafeSendError::PSP22Error(e)
    }
}
