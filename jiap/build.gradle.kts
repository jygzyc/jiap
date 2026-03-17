import com.github.jengelman.gradle.plugins.shadow.tasks.ShadowJar

plugins {
	`java-library`
	alias(libs.plugins.kotlin.jvm)
	alias(libs.plugins.shadow)
	alias(libs.plugins.use.latest.versions)
	alias(libs.plugins.ben.manes.versions)
}

kotlin {
    jvmToolchain(11)
}

val isJadxSnapshot = libs.versions.jadx.get().endsWith("-SNAPSHOT")

dependencies {
    compileOnly(libs.jadx.core) {
        isChanging = false
    }
    compileOnly(libs.jadx.gui) {
        isChanging = false
    }
    compileOnly(libs.jadx.cli) {
        isChanging = false
    }
    implementation(libs.gson)
    implementation(libs.javalin)
    implementation(libs.jackson.databind)
    compileOnly(libs.slf4j.api)

    testImplementation(libs.jadx.smali.input) {
        isChanging = isJadxSnapshot
    }
    compileOnly(libs.logback.classic)
    testImplementation(libs.assertj.core)
    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)
}

sourceSets {
    main {
        resources {
            srcDirs("src/main/resources")
        }
    }
}

tasks.processResources {
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    from("mcp_server") {
        include("jiap_mcp_server.py")
        include("pyproject.toml")
        include("requirements.txt")
        include(".python-version")
        into("mcp")
    }
}

repositories {
    mavenLocal()
    mavenCentral()
    maven(url = "https://s01.oss.sonatype.org/content/repositories/snapshots/")
    google()
    maven(url = "https://jitpack.io")
}

version = System.getenv("JIAP_VERSION") ?: "dev"

// Configure Java compiler to preserve parameter names
tasks.withType<JavaCompile> {
    options.compilerArgs.add("-parameters")
}

// Configure Kotlin compiler to preserve parameter names
tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
    compilerOptions {
        javaParameters = true
    }
}

tasks {
    withType(Test::class) {
        useJUnitPlatform()
    }

    named<Delete>("clean") {
        delete(layout.buildDirectory)
    }
    val shadowJar = withType(ShadowJar::class) {
        archiveClassifier = ""
    }

    // copy result jar into "build/dist" directory
    register<Copy>("dist") {
		group = "build"
        dependsOn(shadowJar)
        dependsOn(withType(Jar::class))

        from(shadowJar)
        into(layout.buildDirectory.dir("dist"))
    }
}