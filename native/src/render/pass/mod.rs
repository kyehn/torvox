//! 渲染循环：帧提交、同步与错误恢复。

mod acquire;
mod bands;
mod frame;
#[cfg(test)]
mod readback;

pub(crate) use acquire::{
    ACQUIRE_DRAIN_TIMEOUT, ACQUIRE_TIMEOUT, AcquireOutcome, drain_in_flight_acquires,
};

#[cfg(test)]
mod tests;
