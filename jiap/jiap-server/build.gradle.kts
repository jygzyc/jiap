plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.shadow)
}

dependencies {
    implementation(project(":jiap-core"))
    implementation(libs.jadx.core) {
        isChanging = false
    }
    implementation(libs.jadx.cli) {
        isChanging = false
    }
    implementation(libs.gson)
    implementation(libs.slf4j.api)
    implementation(libs.logback.classic)
}

tasks.processResources {
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}

tasks {
    named<com.github.jengelman.gradle.plugins.shadow.tasks.ShadowJar>("shadowJar") {
        archiveClassifier = ""
        archiveVersion = project.version.toString()
        mergeServiceFiles()
        manifest {
            attributes("Main-Class" to "jadx.plugins.jiap.server.JiapServerApp")
        }
    }
}
