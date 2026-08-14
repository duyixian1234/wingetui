//! winget 数据模型。

use serde::{Deserialize, Serialize};

/// winget 软件包。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Package {
    /// winget Id，如 "Microsoft.PowerShell"。
    pub id: String,
    /// 包名称。
    pub name: String,
    /// 当前/已装版本；搜索时为匹配版本。
    pub version: String,
    /// 可升级版本（仅升级列表有值）。
    pub available_version: Option<String>,
    /// 来源（如 winget / msstore）。
    pub source: Option<String>,
}
