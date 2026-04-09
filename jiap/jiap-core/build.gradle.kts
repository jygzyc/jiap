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
