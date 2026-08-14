//! 日志屏状态（变更操作实时输出）。

/// 日志屏状态：累积的命令输出行 + 结束结果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LogState {
    /// 变更命令实时 stdout/stderr 行（追加序）。
    pub lines: Vec<String>,
    /// 变更结束后的结果提示。
    pub result: Option<String>,
    /// 变更结束后是否自动返回（由上层决定，这里仅记录）。
    pub done: bool,
}

impl LogState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 追加一行日志。
    pub fn push_line(&mut self, line: String) {
        self.lines.push(line);
    }

    /// 记录结束结果并标记完成。
    pub fn finish(&mut self, result: String) {
        self.result = Some(result);
        self.done = true;
    }
}
