// Embeds the application icon, version info and manifest.
//
// The original requires administrator rights (it starts a kernel driver), and release
// builds do the same. Debug builds run as the invoking user so `cargo run` and
// `cargo test` work from a normal shell; they must be started elevated by hand to reach
// the SMC driver.

fn manifest(level: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="RustyMacsFanControl" type="win32"/>
  <description>Rusty Macs Fan Control</description>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" publicKeyToken="6595b64144ccf1df" language="*" processorArchitecture="*"/>
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="{level}" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2, PerMonitor</dpiAwareness>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
</assembly>"#
    )
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icons/app.ico");
    let debug = std::env::var("PROFILE").map(|p| p == "debug").unwrap_or(false);
    let level = if debug { "asInvoker" } else { "requireAdministrator" };
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icons/app.ico");
    res.set_manifest(&manifest(level));
    res.set("ProductName", "Rusty Macs Fan Control");
    res.set("FileDescription", "Rusty Macs Fan Control");
    res.set("CompanyName", "thesupersupersigma");
    res.set("LegalCopyright", "thesupersupersigma");
    res.compile().expect("failed to compile Windows resources");
}
