//! 把应用图标与版本信息写进 Windows 可执行文件的资源段。
//!
//! 资源管理器里看到的 exe 图标、右键「属性 → 详细信息」里的产品名/版本号，
//! 都来自这里；没有这段资源，exe 会显示默认的齿轮图标，版本信息也是空的。
//! 其它平台不需要，构建脚本直接跳过。

fn main() {
    println!("cargo:rerun-if-changed=assets/logo.ico");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("assets/logo.ico")
        .set("ProductName", "MetaAlign")
        .set("FileDescription", "MetaAlign 元数据批量整理器")
        .set("CompanyName", "MetaAlign contributors")
        .set("LegalCopyright", "MIT License")
        .set_language(0x0804); // 简体中文

    if let Err(error) = resource.compile() {
        // 缺少 rc.exe / windres 时给一句能看懂的提示，而不是让用户对着 linker 报错猜。
        panic!("编译 Windows 资源失败（需要 MSVC 的 rc.exe 或 mingw 的 windres）：{error}");
    }
}
