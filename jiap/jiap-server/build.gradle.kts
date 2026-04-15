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
    val versionFile = File(sourceSets.main.get().output.resourcesDir!!, "version.properties")
    versionFile.writeText("version=${project.version}\n")
}

tasks {
    named<com.github.jengelman.gradle.plugins.shadow.tasks.ShadowJar>("shadowJar") {
        archiveBaseName = "jiap-server"
        archiveClassifier = ""
        archiveVersion = project.version.toString()
        mergeServiceFiles()
        manifest {
            attributes("Main-Class" to "jadx.plugins.jiap.server.JiapServerApp")
        }
    }

    register<Copy>("dist") {
        group = "build"
        dependsOn(shadowJar)
        from(shadowJar)
        into(layout.buildDirectory.dir("dist"))
    }
}
