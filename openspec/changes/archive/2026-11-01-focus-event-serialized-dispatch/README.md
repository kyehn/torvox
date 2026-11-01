# focus-event-serialized-dispatch

焦点事件统一走串行派发通道：切会话补发的 focus-in 不再在会话锁内就地调用 Bridge.focusEvent
