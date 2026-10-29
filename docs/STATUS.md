## 未被解决的问题（禁止修改本文件）

基于 `15489682a06240ffa1e43dae12af05d6f787a995`

- 上下滑动晃动
- 使用 kitten icat 显示图像，然后 clear，其他被清除但是图像依旧显示，第二次clear图像被清除
- 全面屏上滑手势会触发修饰栏按键动画（仅动画）
- 通常

  ```text
  …/user/0/com.termux/files/home/project $

  ```

  prompt下面显示输入提示符
  概率性

  ```text
  …/user/0/com.termux/files/home/project $
  ```

  需要手动下滑

- 概率性，较多输出时（不算太多）或io多时或其他情况终端显示可能死亡（剩余输出不可见），刷新终端 输入回车 没有新输出，再输入clear后正常
