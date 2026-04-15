plugins {
    alias(libs.plugins.kotlin.jvm)
}

dependencies {
    implementation(libs.gson)
    implementation(libs.javalin)
    implementation(libs.jackson.databind)
    compileOnly(libs.jadx.core) {
        isChanging = false
    }
    compileOnly(libs.slf4j.api)
    compileOnly(libs.logback.classic)
}

tasks.processResources {
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
    val resourcesDir = sourceSets.main.get().output.resourcesDir!!
    val versionFile = File(resourcesDir, "version.properties")
    versionFile.parentFile.mkdirs()
    versionFile.writeText("version=${project.version}\n")
}
