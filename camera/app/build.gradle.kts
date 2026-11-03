import com.google.protobuf.gradle.proto
import org.gradle.api.DefaultTask
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.file.RegularFileProperty
import org.gradle.api.tasks.InputFile
import org.gradle.api.tasks.OutputDirectory
import org.gradle.api.tasks.PathSensitive
import org.gradle.api.tasks.PathSensitivity
import org.gradle.api.tasks.TaskAction
import org.jetbrains.kotlin.gradle.tasks.KotlinCompile
import java.time.LocalDate
import java.time.format.DateTimeFormatter

abstract class GenerateServerPortsTask : DefaultTask() {
    @get:InputFile
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val generatorScript: RegularFileProperty

    @get:InputFile
    @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val serverSpec: RegularFileProperty

    @get:OutputDirectory
    abstract val outputDirectory: DirectoryProperty

    @TaskAction
    fun generate() {
        val process = ProcessBuilder(
            "python",
            generatorScript.get().asFile.absolutePath,
            "--camera-output",
            outputDirectory.get().asFile.absolutePath,
        ).inheritIO().start()
        check(process.waitFor() == 0) { "Failed to generate camera server port constants." }
    }
}

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.protobuf)
    alias(libs.plugins.ksp)
    alias(libs.plugins.hilt)
}

val enableNatsLog = providers.gradleProperty("enableNatsLog")
val releaseVersion = providers.environmentVariable("ST_VERSION")
val releaseCommitShortHash = providers.environmentVariable("ST_COMMIT_SHORT_HASH")

// Every release signs the APK with one key (.github/workflows/release.yaml) so that the launcher
// can update the app on the phones: Android refuses an update signed with another key.
// Without the variables, the release build is unsigned as before.
val releaseKeystore = providers.environmentVariable("CAMERA_KEYSTORE_FILE")
val serverPortsOutput = layout.buildDirectory.dir("generated/source/serverPorts/main")
val generateServerPorts = tasks.register<GenerateServerPortsTask>("generateServerPorts") {
    val repositoryRoot = rootProject.projectDir.parentFile
    generatorScript.set(repositoryRoot.resolve("scripts/generate_server_ports.py"))
    serverSpec.set(repositoryRoot.resolve("api/spec/servers.yaml"))
    outputDirectory.set(serverPortsOutput)
}

// The release (.github/workflows/release.yaml) gives the version like the other modules, e.g.
// '2026-10-20.3' (see ReleaseTag in xtask/src/release.rs). The launcher updates the app with
// `adb install -r`, which refuses a smaller versionCode, so the versionCode is YYYYMMDD * 100 + N:
// it increases with every release and stays below the limit of Android (2100000000) until 2100.
// Local builds have no version.
val (appVersionCode, appVersionName) = if (releaseVersion.isPresent) {
    val version = releaseVersion.get()
    val date = LocalDate.parse(version.substringBefore('.'))
    val releaseNumber = version.substringAfter('.').toInt()
    check(releaseNumber < 100) { "release number of ST_VERSION must be less than 100: $version" }
    check(releaseCommitShortHash.isPresent) { "ST_COMMIT_SHORT_HASH must be set with ST_VERSION" }
    val versionCode = date.format(DateTimeFormatter.BASIC_ISO_DATE).toInt() * 100 + releaseNumber
    versionCode to version
} else {
    1 to "no version"
}

android {
    namespace = "com.taichi765.struckoutCameraApp"
    compileSdk {
        version = release(37)
    }

    defaultConfig {
        // 元のアプリと同じ端末に共存できるよう、ID を分ける。
        applicationId = "com.taichi765.struckoutCameraApp.highspeed"
        minSdk = 28
        targetSdk = 36
        versionCode = appVersionCode
        versionName = appVersionName
        buildConfigField(
            "String",
            "COMMIT_SHORT_HASH",
            "\"${releaseCommitShortHash.orElse("").get()}\""
        )

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ksp {
            arg("room.schemaLocation", "$projectDir/schemas")
        }
    }

    signingConfigs {
        if (releaseKeystore.isPresent) {
            create("release") {
                storeFile = file(releaseKeystore.get())
                storePassword = providers.environmentVariable("CAMERA_KEYSTORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("CAMERA_KEY_ALIAS").get()
                keyPassword = providers.environmentVariable("CAMERA_KEY_PASSWORD").get()
            }
        }
    }

    buildTypes {
        debug {
            buildConfigField("boolean", "ENABLE_NATS_LOG", enableNatsLog.orElse("false").get())
        }

        release {
            if (releaseKeystore.isPresent) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            buildConfigField("boolean", "ENABLE_NATS_LOG", enableNatsLog.orElse("true").get())
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
    sourceSets {
        getByName("main") {
            proto {
                srcDir("../../api/proto/")
            }
        }
    }
}

androidComponents.onVariants { variant ->
    val kotlinSources = variant.sources.kotlin
        ?: error("Kotlin source directory is unavailable for ${variant.name}.")
    kotlinSources.addGeneratedSourceDirectory(
        generateServerPorts,
        GenerateServerPortsTask::outputDirectory,
    )
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.video)
    implementation(libs.androidx.camera.view)
    implementation(libs.androidx.nav.compose)
    implementation(libs.androidx.datastore)
    implementation(libs.protobuf.javalite)
    implementation(libs.protobuf.kotlin.lite)
    implementation(libs.timber)
    implementation(libs.hilt)
    implementation(libs.hilt.navigation.compose)
    implementation(libs.room.runtime)
    implementation(libs.room.ktx)
    implementation(project(":opencv"))
    testImplementation(libs.robolectric)
    testImplementation(libs.mockk)
    testImplementation(libs.mockk.agent)
    testImplementation(libs.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)
    testImplementation(libs.kotlinx.coroutines.test)
    testImplementation(libs.room.testing)
    testImplementation(libs.turbine)
    androidTestImplementation(libs.junit4)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    debugImplementation(libs.androidx.compose.ui.tooling)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    ksp(libs.hilt.compiler)
    ksp(libs.room.compiler)
    annotationProcessor(libs.room.compiler)
}

protobuf {
    // Use system-installed protoc to share the same binary with Rust
    generateProtoTasks {
        all().forEach { task ->
            task.builtins {
                create("java") {
                    option("lite")
                }
                create("kotlin") {
                    option("lite")
                }
            }
        }
    }
}

tasks.withType<KotlinCompile>().configureEach {
    compilerOptions {
        freeCompilerArgs.add("-Xannotation-default-target=param-property")
    }
}

tasks.withType<Test>().configureEach {
    useJUnitPlatform()
}
