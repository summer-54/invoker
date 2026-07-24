pub use super::server_lib::stream::{
    Stream,
    auth::{InvokerToManager as AuthOutgo, ManagerToInvoker as AuthIncome, NAME as AUTH_NAME},
    judge::{InvokerToManager as JudgeOutgo, ManagerToInvoker as JudgeIncome, NAME as JUDGE_NAME},
    master::{
        InvokerToManager as MasterOutgo, ManagerToInvoker as MasterIncome, NAME as MASTER_NAME,
    },
};
