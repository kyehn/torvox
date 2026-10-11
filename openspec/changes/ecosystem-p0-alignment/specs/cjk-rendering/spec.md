# Spec Delta

## ADDED Requirements

### Requirement: 行尾孤立代理项不越界

宽字符宽度判定在行尾遇到孤立高代理项 MUST 有长度守卫，MUST NOT 越界读取下一个码元或将其误配对；孤立代理项按宽度 1 处理。

#### Scenario: 行尾孤立高代理项不越界

- **WHEN** 行尾最后一个码元为无配对低代理项的高代理项
- **THEN** 宽度判定返回 1 且不读取行外内存，不崩溃
