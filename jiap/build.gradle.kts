plugins {
    alias(libs.plugins.kotlin.jvm) apply false
    alias(libs.plugins.shadow) apply false
    alias(libs.plugins.use.latest.versions)
    alias(libs.plugins.ben.manes.versions)
}

repositories {
    mavenLocal()
    mavenCentral()
    maven(url = "https://s01.oss.sonatype.org/content/repositories/snapshots/")
    google()
    maven(url = "https://jitpack.io")
}

val projectVersion = System.getenv("JIAP_VERSION") ?: "dev"
version = projectVersion

subprojects {
    version = projectVersion

    repositories {
        mavenLocal()
        mavenCentral()
        maven(url = "https://s01.oss.sonatype.org/content/repositories/snapshots/")
        google()
        maven(url = "https://jitpack.io")
    }

    pluginManager.withPlugin("org.jetbrains.kotlin.jvm") {
        configure<org.jetbrains.kotlin.gradle.dsl.KotlinProjectExtension> {
            jvmToolchain(11)
        }
    }

    tasks.withType<JavaCompile> {
        options.compilerArgs.add("-parameters")
    }

    tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> {
        compilerOptions {
            javaParameters = true
        }
    }

    tasks.withType<Test> {
        useJUnitPlatform()
    }
}

tasks.register<Copy>("dist") {
    group = "build"
    dependsOn(":jiap-plugin:dist")
    dependsOn(":jiap-server:dist")

    from("${project(":jiap-plugin").projectDir}/build/dist")
    from("${project(":jiap-server").projectDir}/build/dist")
    into(layout.buildDirectory.dir("dist"))
}
