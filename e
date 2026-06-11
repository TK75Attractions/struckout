[33m365d09e[m[33m ([m[1;36mHEAD[m[33m -> [m[1;32mmain[m[33m)[m chore: Fix the collision.proto and generated C# file
[33m8531945[m[33m ([m[1;31morigin/main[m[33m, [m[1;31morigin/HEAD[m[33m)[m Merge branch 'main' of https://github.com/TK75Attractions/struckout
[33mdff4c9c[m Merge branch 'UnityProject'
[33m57dd4c5[m[33m ([m[1;31morigin/UnityProject[m[33m, [m[1;32mUnityProject[m[33m)[m chore(Unity): Add Asmdef in Bootstrap
[33mf8c5267[m chore(Unity): Adjust Asmdef
[33m74d8243[m feat: Add PacketRouter and Connect between Rust and Unity
[33m07ca16e[m refactor(tracker): Rename FrameSocket -> UdpTransport
[33m15495fe[m chore(tracker/deps): Use lower version of nalgebra to integrate with kfilter
[33m10d21d4[m feat: Make Test Listener Build Stable
[33m3e614e2[m chore: Replace NetworkPacket to proto
[33mb7cdbb3[m chore: Add forgetten proto file
[33m98ce605[m feat: Add new proto type for debug
[33me3bc29b[m chore(testTCP): Remove tracking of target Directory
[33mebec066[m feat(master): Add SQLite migration file
[33m41ad7f2[m chore(master): Add target/ to .gitignore
[33m29977f0[m feat(Unity): Add ReceiveDataAsync()
[33m9590a6b[m feat(Unity): Add ProtoDeserializer
[33m4c35603[m fix(camera): Handle exception when TCP connection is unexpectedly closed
[33m0d64327[m fix(camera): Bind UDP socket to a port before starting camera
[33m8623b1a[m fix: Asmdef References cause Error
[33m287b6ca[m chore: Delete the old CollisionPoint and adjust other class
[33mdf09265[m feat: Add Protobuf to UnityProject and Make CollisionPoint.proto
[33m8f85ace[m feat: Add Test Listner for Rust
[33m59a9edf[m fix: Use BytesMut::zeroed() instead of BytesMut::with_capacity()
[33me6c071d[m feat(camera/ui): Enable retrying TCP connection
[33m7eaea02[m refactor(camera/ui): Remove unnecessary preview
[33mb77c300[m fix(ball_watcher): Inifinitely loop for accepting new connection
[33m7912bff[m fix(camera): Don't close TCP socket
[33mc2fe91f[m feat(camera/ui): Use ImeAction.Next for keyboard input in CameraLocationScreen
[33mb090773[m feat: Add protobuf message for communication between ball_watcher and projector
[33m4edc744[m Add New Rust File for the test of the connecition between Unity C# and Rust
[33m9718a96[m test: Add test to check if data length is de/serialized between kotlin and rust
[33mccaaf30[m feat: Add server -> client communication
[33md821bf3[m fix: Add a header to tell the data length before protobuf message in TCP
[33me3840a9[m refactor: Separate WorldDirectionCalculator to its own file
[33m4851a6b[m feat: Add TCP communication (ATM client -> server only)
[33m5da2aa5[m feat(ball_watcher): Add tracing-subscriber
[33m40aa8ea[m fix: Remove redundant navigation
[33med92353[m chore: Format proto file
[33mf9339b3[m chore: Add recommended extension for protobuf
[33m501d23b[m chore: Remove BLE-related codes
[33m9a1976f[m refactor: Locate data classes related to camera in camera/types/
[33m43921cd[m AddNewUnityProject
[33mafdab3d[m feat: Progress on migrating from BLE to UDP/TCP
[33m6838c61[m chore: Add system_architecture.md
[33m18bc95e[m chore: Update README.md
[33mde53817[m chore: Add README.md to describe project structure
[33ma520a1b[m refactor: Use protobuf-gradle-plugin to generate binding from *.proto
[33m31374a0[m feat: Add basic UDP and TCP
[33m32a9bd8[m feat: Extend protobuf definitions
[33md47004b[m feat: Generate kotlin code from protobuf
[33mffbd35e[m feat: Progress on migrating to TCP and UDP
[33m1311516[m fix: Add missing import
[33m5fcf3b0[m refactor: Run ball watcher on PC
[33m2d025cb[m refactor: Restructure directories
[33mc9dcf4b[m chore: Update README
[33m37e3a1e[m feat: Add coordinate calculation logic
[33mec5c480[m chore: Install just-lsp
[33m9104e7b[m chore(deps): Add features for defmt and log
[33m8835067[m chore(just): Use variables to organize
[33m2a95ba7[m chore: Add more recipes in justfile
[33mb93d56d[m refactor: Apply clippy suggestions
[33m1cd04d0[m chore: Use just to run tasks
[33m6f8e802[m chore: Separate crates for testing pure-rust things (like unit tests in Android project)
[33m3ff3b43[m refactor(ui): Improve alignment for BleInfoView
[33mc960c1d[m feat: Start working on coordinate calculation logic
[33m3bca46a[m chore: Downgrade rust-analyzer version
[33mf274bf8[m fix: Use write_without_response instead of write for GATT characteritics
[33m978d3f6[m feat: Enable BLE Connection
[33m2a7854d[m amend! Move README.md to project root
[33m9fb1fb0[m fix: Handle permission properly
[33mc04d8a1[m chore: Move README.md to project root
[33m6cd205b[m fix: Fix issue on calculating world direction vector
[33m05e3f78[m chore(cargo): Add alias to run on esp32s3
[33m8bdeb38[m chore: Add documentation about direction calculation
[33md8a2b3c[m chore(ci): Don't deny unused variable
[33mb8b7672[m chore(ci): Set proper target again
[33me557268[m chore(ci): Set proper target for clippy command
[33m74a350e[m chore(ci): Fix typo in cargo command argument
[33m44f9cd7[m refactor: Apply clippy suggestion
[33mf2cc863[m feat: Progress on Android side
[33ma9e47bb[m chore(deps): Bump composeBom version
[33md120cd8[m feat: Use androix.navigation.compose
[33m66878ac[m amend: Commit untracked files
[33ma0bccbb[m feat: Progress on Ble and refactoring
[33m2cc575a[m chore: Use 'tty' subsystem instead of 'usb' in udev rules
[33m3f389d2[m feat: Migrate from Kotlin-BLE-Library to Kable
[33m855d1b7[m feat: Add basic structure for BLE
[33me286da9[m chore: Add configurations for esp32
[33mfe14275[m feat: Use devcontainer for struckout-projector
[33m9db1653[m feat: Start working on projector image
[33m298db1b[m feat: Start working on BLE client
[33m51318ac[m feat: Add direction vector calculation logic
[33md326e4e[m chore(deps): Bump agp version
[33m2c2a547[m chore: Add line feed in README.md
[33m6466e52[m chore(ci): Fix command name
[33m21d1910[m chore(ci): Use proper feature in clippy command
[33ma4bbc6c[m chore(ci): Don't use parentheses in matrix name
[33mccc468b[m fix: Mark as todo!()
[33mdb4fabf[m chore(mcu): Don't use path dependency to simplify workflow
[33m1382486[m fix: Set workdir properly
[33me2bfff5[m chore(ci): Add GitHub Actions
[33m888fff5[m chore(android): Bump agp version
[33ma218f36[m fix: Handle gatt event concurrently
[33m965a27b[m chore: Downgrade r-a version
[33md72278a[m chore: Use devcontainer options over raw docker cli options
[33m9a4c2a7[m refactor: Organize imodules
[33m09d7396[m chore: Improve devcontainer
[33me643079[m chore: Fix feature flag
[33mb7e65e4[m chore: Add log feature
[33meb59400[m fix: Set max connection properly
[33m79caf5c[m fix: Allocate initial buffers on .bss, not .dram2_uninit
[33mdc900ec[m chore: Fix rust-analyzer settings
[33m81789ae[m chore: Add vscode settings
[33m8ed3613[m chore: コンテナ化
[33mbe18c68[m feat: esp32s3に対応
[33m567c787[m feat: サービスの実装など
[33m87e1b6e[m chore(android): Update agp
[33m430bc48[m chore: Use embassy, trouble etc. instead of esp-idf-svc
[33m9145007[m chore: Add shell script to install OpenCV SDK automatically
[33mba71bb2[m chore: Update .gitignore to actually ignore opencv/
[33m4273138[m chore: Add opencv/ to .gitignore
[33mef3001b[m initial commit
