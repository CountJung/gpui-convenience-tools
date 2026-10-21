use serde::{Deserialize, Serialize};

/// 시간대만 영속화한다. 변환 입력과 결과는 일시적인 작업 데이터다.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TimestampConfig {
    pub utc: bool,
}
