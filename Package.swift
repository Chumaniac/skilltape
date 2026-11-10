// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SkillTapeDesktopHost",
    platforms: [.macOS(.v13)],
    products: [.executable(name: "SkillTapeDesktopHost", targets: ["SkillTapeDesktopHost"])],
    targets: [
        .executableTarget(
            name: "SkillTapeDesktopHost",
            path: "desktop",
            exclude: ["README.md", "app.json", "build_app.py", "tests"]
        )
    ]
)
