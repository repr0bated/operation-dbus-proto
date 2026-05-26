# Production Security and Quality Audit: op-deployment

## 1. Vulnerability Findings

### [CRITICAL] Memory Safety & Undefined Behavior via Padded Buffer Violation in `simd_json::from_str`
- **Location:** `crates/op-deployment/src/image_manager.rs:327` and `crates/op-deployment/src/image_manager.rs:344`
- **Impact:** Memory corruption, heap out-of-bounds read/write, or segmentation faults (Undefined Behavior).
- **Description:** The `ImageManager` parses metadata files using `unsafe { simd_json::from_str(...) }`. The `simd-json` parsing architecture requires that input string/slice buffers possess padding (typically `simd_json::SIMDJSON_PADDING`, which is 32 or 64 bytes) beyond the JSON payload's end. This is because the SIMD vector instructions read chunks of 32/64 bytes at a time and can perform out-of-bounds memory accesses if the buffer is unpadded. Since `content` is a standard `String` populated by `async_fs::read_to_string`, it does not guarantee this trailing addressable padding. Deserializing unpadded buffers via `unsafe` in-place parsing methods results in direct memory safety risks.
- **Remediation:** Avoid `unsafe` parsing or copy the string into a padded buffer using `simd_json::to_padded_string` before parsing.

---

### [CRITICAL] Arbitrary Directory Deletion via Path Traversal in `delete_image`
- **Location:** `crates/op-deployment/src/image_manager.rs:386-421` (specifically lines 387, 410-413, and 418)
- **Impact:** Uncontrolled file/directory deletion, leading to denial of service (DoS) or host system destruction.
- **Description:** The `delete_image` method resolves the target directory using `self.images_dir.join(image_name)`. Because the `image_name` argument is not sanitized or checked for path traversal components (such as `../`), a directory path outside of the designated `images_dir` can be targeted. If an attacker controls or influences the `image_name` argument, they can invoke `delete_image` with a path like `../../../../usr/local` or `../../etc`, causing the manager to execute `btrfs subvolume delete` or `async_fs::remove_dir_all` on highly sensitive host system directories.
- **Remediation:** Sanitize `image_name` to ensure it does not contain parent directory traversal sequences (`..`), or canonicalize the resolved path and verify that it remains a strict subdirectory of `self.images_dir`.

---

### [HIGH] Arbitrary Host File Symlink Injection and Disclosure
- **Location:** `crates/op-deployment/src/image_manager.rs:248-261` (in `find_file_in_previous_images`) and `crates/op-deployment/src/image_manager.rs:135-143` (in `create_image`)
- **Impact:** Symlink-based host file disclosure and read/write access.
- **Description:** 
  1. `find_file_in_previous_images` resolves file locations using `image.path.join(file_name)`. The `image.path` value is trust-deserialized directly from `.image-metadata.json` on disk. If an attacker tampers with this JSON file, they can inject an absolute path pointing to a system directory (e.g., `/etc`).
  2. In `create_image`, when a file matches this previous metadata entry, the code calculates a relative path using `calculate_relative_path` and establishes a symbolic link using `std::os::unix::fs::symlink`. Because there are no boundary checks, this generates a symlink that escapes the container/deployment image root, mapping directly to arbitrary target host files (e.g., `/etc/passwd`).
- **Remediation:** Validate that all resolved previous image paths, target files, and symlink destinations strictly reside within the designated deployment subdirectories and do not escape the base image directory root.

---

### [HIGH] Path Traversal in Image Creation and Snapshots
- **Location:** `crates/op-deployment/src/image_manager.rs:105` and `crates/op-deployment/src/image_manager.rs:283`
- **Impact:** Directory creation and snapshot generation outside of the designated image directories.
- **Description:** In both `create_image` and `create_image_snapshot`, `image_name` is joined directly to `images_dir` without path traversal sanitization. This allows malicious directory structures to be created outside the sandbox, and exposes the BTRFS shell-free execution commands to arbitrary paths on the file system.
- **Remediation:** Validate that the resolved `image_path` behaves as a safe subdirectory of `self.images_dir`.

---

## 2. Schema-as-Code Violations

- **Location:** `crates/op-deployment/src/image_manager.rs:18-38`
- **Violation:** Ad-hoc serialized metadata contracts.
- **Description:** The `ImageMetadata` and `FileEntry` data contracts are expressed directly as raw, unstructured Rust structs with derived Serde serialization, and are saved directly as `.image-metadata.json` files on disk (lines 188-190). These structures govern critical state logic for image symlinking, snapshot references, and deployment state, but lack versioned Protocol Buffer schemas or OSCAL-compliant definitions. Ad-hoc serializations increase the risk of parser breakages, structural drift, and deserialization vulnerabilities when the storage formats change across deployment versions.
- **Remediation:** Define `ImageMetadata` and its child elements inside a versioned `.proto` file (e.g. `op_deployment/v1/metadata.proto`), and compile it into Rust structures at build time via `prost-build` or `tonic-build`.

---

## 3. Build & Quality Audit Checklist

### Cargo.toml Workspace Analysis
- **Edition:** The `op-deployment` crate inherits `edition = "2021"` from the workspace package configurations (`edition.workspace = true` in `crates/op-deployment/Cargo.toml`).
- **Rust Version:** The `rust-version` field is omitted from both the workspace configuration (`Cargo.toml`) and the local crate configuration (`crates/op-deployment/Cargo.toml`). This allows the crate to be compiled with arbitrary compiler toolchains, which may introduce compatibility or safety regressions.
- **Bins & Examples:** No binary targets (`[[bin]]`) or executable examples are defined inside the `op-deployment` crate. It operates exclusively as a library interface.
- **Workspace Inheritance vs. Local Overrides:** The crate uses strict workspace inheritance for all external dependencies (e.g., `tokio`, `serde`, `simd-json`, `anyhow`, etc., are configured with `workspace = true`). There are no local dependency overrides defined inside `crates/op-deployment/Cargo.toml`.

### Codegen and Build Script Review
- **build.rs Presence:** No `build.rs` or codegen script is provided or defined within the files for `crates/op-deployment`. There are no compilation-stage shell execution risks or code generation routines active inside this specific crate.
- **Schema Compilation:** 
  - The `op-deployment` crate does not trigger any compilation of Protocol Buffers at build time or runtime.
  - Review of the workspace `Cargo.lock` reveals that companion crates in the same repository workspace (such as `op-cache`, `op-chat`, `op-cognitive-mcp`, `op-mcp`, `op-services`, etc.) pull in `prost-build` or `tonic-build` for codegen, but `op-deployment` operates completely independently of these protobuf pipelines. No raw generated Rust files are committed within the provided `crates/op-deployment` directory scope.