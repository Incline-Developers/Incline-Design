//! SCIP's return codes (`type_retcode.h`). `SCIP_OKAY` is `Ok`; every other
//! code is an [`Retcode`] error, propagated with `?` where C writes
//! `SCIP_CALL`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retcode {
    Error,
    NoMemory,
    ReadError,
    WriteError,
    NoFile,
    FileCreateError,
    LpError,
    NoProblem,
    InvalidCall,
    InvalidData,
    InvalidResult,
    PluginNotFound,
    ParameterUnknown,
    ParameterWrongType,
    ParameterWrongVal,
    KeyAlreadyExisting,
    MaxDepthLevel,
    BranchError,
    NotImplemented,
}

pub type ScipResult<T = ()> = Result<T, Retcode>;
