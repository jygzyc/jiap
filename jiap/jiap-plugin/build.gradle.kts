plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.shadow)
}

dependencies {
    implementation(project(":jiap-core"))
    compileOnly(libs.jadx.core) {
        isChanging = false
    }
    compileOnly(libs.jadx.gui) {
        isChanging = false
    }
    compileOnly(libs.jadx.cli) {
        isChanging = false
    }
    compileOnly(libs.javalin)
    compileOnly(libs.gson)
    compileOnly(libs.slf4j.api)
    compileOnly(libs.logback.classic)
}

tasks.processResources {
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}

tasks {
    named<com.github.jengelman.gradle.plugins.shadow.tasks.ShadowJar>("shadowJar") {
        archiveClassifier = ""
        archiveVersion = project.version.toString()
        mergeServiceFiles()
    }
}
