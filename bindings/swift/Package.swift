// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "Kungfu",
    products: [
        .library(name: "Kungfu", targets: ["Kungfu"]),
    ],
    targets: [
        .systemLibrary(
            name: "CKungfu",
            path: "Sources/CKungfu"
        ),
        .target(
            name: "Kungfu",
            dependencies: ["CKungfu"],
            path: "Sources/Kungfu"
        ),
    ]
)
