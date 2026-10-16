//! 探测 SystemDefined CGEvent 私有字段与 NSEvent subtype/data1/data2 的对应关系。
//!
//! 背景：tap 回调线程里不能调用 `NSEvent::eventWithCGEvent`——新版 macOS 上
//! Caps Lock 切换输入法的系统事件会在转换过程中进入 TSM 输入源代码并断言
//! 主队列（`dispatch_assert_queue_fail`），在 tap 线程直接 SIGTRAP 整个进程。
//! 因此 `macos.rs` 在 CGEvent 字段层面解析 SystemDefined 事件，本文件用
//! 两个方向验证它依赖的私有字段映射：
//!
//! 1. `system_defined_private_fields`（常规运行）：用 AppKit 构造已知
//!    subtype/data1/data2 的 SystemDefined NSEvent，取其 CGEvent 后逐字段
//!    读出，断言映射仍为 55=类型、83=subtype、149=data1、150=data2。
//! 2. `system_defined_conversion_symmetry`（默认忽略，需主线程）：正向把
//!    私有字段写成魔数后转成 NSEvent，确认读出的 subtype/data1 与之一致。
//!    运行方式：
//!    `RUST_TEST_THREADS=1 cargo test -p tfp-input-stats -- --ignored --nocapture`
//!
//! 实测注意（2026-10-16，macOS 26）：
//! - 公开字段 `kCGMouseEventSubtype`(7) 写入/读取都到不了 NSEvent 的 subtype，
//!   真实值存在私有字段里；
//! - 私有字段 55 是内部类型槽，写入大数会抛出 NSException
//!   （"unrecognized type is …"）直接中止进程。

use objc2::encode::{Encode, RefEncode};
use objc2::{class, msg_send, rc::autoreleasepool, runtime::AnyObject};
use objc2_app_kit::NSEvent;
use objc2_core_graphics::CGEvent;
use std::ffi::c_void;
use std::ptr::NonNull;

type Ref = *mut c_void;

#[repr(C)]
struct ProbePoint {
    x: f64,
    y: f64,
}

unsafe impl Encode for ProbePoint {
    const ENCODING: objc2::encode::Encoding = objc2::encode::Encoding::Struct(
        "CGPoint",
        &[
            objc2::encode::Encoding::Double,
            objc2::encode::Encoding::Double,
        ],
    );
}

unsafe impl RefEncode for ProbePoint {
    const ENCODING_REF: objc2::encode::Encoding = objc2::encode::Encoding::Pointer(&Self::ENCODING);
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: Ref) -> Ref;
    fn CGEventSetType(event: Ref, kind: u32);
    fn CGEventSetIntegerValueField(event: Ref, field: u32, value: i64);
    fn CGEventGetIntegerValueField(event: Ref, field: u32) -> i64;
    fn CFRelease(cf: Ref);
}

const SYSTEM_DEFINED: u32 = 14;
/// 私有字段：内部类型槽（NSEvent type）。写入大数会抛 NSException，勿随意写。
const PRIVATE_TYPE: u32 = 55;
/// 私有字段：NSEvent subtype
const PRIVATE_SUBTYPE: u32 = 83;
/// 私有字段：NSEvent data1
const PRIVATE_DATA1: u32 = 149;
/// 私有字段：NSEvent data2
const PRIVATE_DATA2: u32 = 150;

/// 构造一个 subtype=8 的 SystemDefined NSEvent，返回它背后的 CGEvent 裸指针。
///
/// `-CGEvent` 返回的是自动释放对象，这里不释放（测试进程泄漏两个对象无妨）。
unsafe fn system_defined_cg_event(subtype: i16, data1: isize, data2: isize) -> Ref {
    let cls = class!(NSEvent);
    let event: *mut AnyObject = msg_send![
        cls,
        otherEventWithType: 14u64,
            location: ProbePoint { x: 0.0, y: 0.0 },
            modifierFlags: 0u64,
            timestamp: 0.0f64,
            windowNumber: 0isize,
            context: std::ptr::null_mut::<AnyObject>(),
            subtype: subtype,
            data1: data1,
            data2: data2,
    ];
    assert!(!event.is_null(), "NSEvent otherEventWithType failed");
    let cg: *mut CGEvent = msg_send![event, CGEvent];
    assert!(!cg.is_null(), "NSEvent CGEvent is null");
    cg.cast::<c_void>()
}

/// 常规回归：AppKit 构造的事件里，subtype/data1/data2 就在私有字段 83/149/150。
#[test]
fn system_defined_private_fields() {
    unsafe {
        // 键值 16 (PLAY) 按下且非重复：data1 = 0x0010_0A00；第二个事件换成键值 20。
        // data2 用 i32 内的正数：字段读回时按 32 位符号扩展
        let cg = system_defined_cg_event(8, 0x0010_0A00, 0x1234_5678);
        let cg2 = system_defined_cg_event(8, 0x0014_0A01, 0x5678_1234);

        assert_eq!(CGEventGetIntegerValueField(cg, PRIVATE_TYPE), 14);
        assert_eq!(CGEventGetIntegerValueField(cg, PRIVATE_SUBTYPE), 8);
        assert_eq!(CGEventGetIntegerValueField(cg, PRIVATE_DATA1), 0x0010_0A00);
        assert_eq!(CGEventGetIntegerValueField(cg, PRIVATE_DATA2), 0x1234_5678);
        assert_eq!(CGEventGetIntegerValueField(cg2, PRIVATE_SUBTYPE), 8);
        assert_eq!(CGEventGetIntegerValueField(cg2, PRIVATE_DATA1), 0x0014_0A01);
        assert_eq!(CGEventGetIntegerValueField(cg2, PRIVATE_DATA2), 0x5678_1234);

        // 公开字段 7 (kCGMouseEventSubtype) 读不到 subtype，别用它做判断
        assert_ne!(CGEventGetIntegerValueField(cg, 7), 8);
    }
}

/// 正向对称性（需主线程，默认忽略）：把私有字段写成魔数后转 NSEvent，
/// 读出的 subtype/data1 应与魔数一致。升级 macOS 后可用它复核映射是否漂移。
#[test]
#[ignore = "需在主线程运行：RUST_TEST_THREADS=1 cargo test -p tfp-input-stats -- --ignored --nocapture"]
fn system_defined_conversion_symmetry() {
    let convert = |field: u32, value: i64| -> (i16, isize, isize) {
        unsafe {
            let ev = CGEventCreate(std::ptr::null_mut());
            assert!(!ev.is_null(), "CGEventCreate failed");
            CGEventSetType(ev, SYSTEM_DEFINED);
            CGEventSetIntegerValueField(ev, field, value);
            let out = autoreleasepool(|_| {
                let raw = NonNull::new(ev).unwrap().cast::<CGEvent>();
                match NSEvent::eventWithCGEvent(raw.as_ref()) {
                    Some(sys) => (sys.subtype().0, sys.data1(), sys.data2()),
                    None => (-1, -1, -1),
                }
            });
            CFRelease(ev);
            out
        }
    };
    // type 槽（55）只能写合法值 14；subtype/data1/data2 写入后应原样读出
    let (_, d1, _) = convert(PRIVATE_DATA1, 0x0010_0A00);
    assert_eq!(d1, 0x0010_0A00);
    let (st, _, _) = convert(PRIVATE_SUBTYPE, 8);
    assert_eq!(st, 8);
    let (_, _, d2) = convert(PRIVATE_DATA2, 0x1234_5678);
    assert_eq!(d2, 0x1234_5678);
    let (_, d1, _) = convert(PRIVATE_DATA1, 0x0014_0A01);
    assert_eq!(d1, 0x0014_0A01);
    assert_eq!(convert(PRIVATE_TYPE, 14).0, 0);
}
