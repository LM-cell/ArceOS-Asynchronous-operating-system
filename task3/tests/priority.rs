// 导入断言工具用于测试验证
use pretty_assertions::assert_eq;
// 导入优先级相关的演示函数
use task3_exec_flow_memory::priority::{
    preemptive_priority_demo_trace, priority_demo_completion_order,
};

/// 测试用例：验证高优先级任务在低优先级任务之前完成
/// 
/// 该测试演示非抢占式调度器中的优先级处理：
/// - 高优先级任务先执行完成
/// - 低优先级任务后执行完成
#[test]
fn high_priority_task_completed_before_low_priority() {
    // 获取任务完成的执行顺序
    let order = priority_demo_completion_order();

    // 验证执行顺序符合优先级规则：高优先级任务（high-1, high-2）优先于低优先级任务（low-1, low-2）
    assert_eq!(order, vec!["high-1", "high-2", "low-1", "low-2"]);
}

/// 测试用例：验证高优先级任务能够抢占低优先级任务
/// 
/// 该测试演示抢占式调度器中的优先级处理：
/// - 低优先级长任务开始执行后被中断
/// - 高优先级短任务抢占CPU
/// - 低优先级长任务继续执行直到完成
#[test]
fn high_priority_task_preempts_low_priority_task() {
    // 获取任务调度的执行追踪信息
    let trace = preemptive_priority_demo_trace();

    // 验证任务分派顺序：低优先级长任务被中断，高优先级短任务插队执行，低优先级长任务继续完成
    assert_eq!(
        trace.dispatch_order(),
        vec!["low-long", "high-short", "low-long", "low-long"]
    );
    // 验证任务完成顺序：高优先级短任务先完成，低优先级长任务后完成
    assert_eq!(trace.completion_order, vec!["high-short", "low-long"]);
}
