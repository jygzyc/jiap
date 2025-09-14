import com.github.jengelman.gradle.plugins.shadow.tasks.ShadowJar

plugins {
	alias(libs.plugins.kotlin.jvm)
	alias(libs.plugins.shadow)
	alias(libs.plugins.use.latest.versions)
	alias(libs.plugins.ben.manes.versions)
}

kotlin {
    jvmToolchain(11)
}

val jadxVersion = "1.5.2"
val isJadxSnapshot = jadxVersion.endsWith("-SNAPSHOT")

dependencies {
    compileOnly("io.github.skylot:jadx-core:$jadxVersion") {
        isChanging = false
    }
    compileOnly("io.github.skylot:jadx-gui:$jadxVersion"){
        isChanging = false
    }
    compileOnly("io.github.skylot:jadx-cli:$jadxVersion"){
        isChanging = false
    }
    implementation("io.javalin:javalin:6.7.0")

	testImplementation("io.github.skylot:jadx-smali-input:$jadxVersion") {
        isChanging = isJadxSnapshot
    }
	testImplementation("ch.qos.logback:logback-classic:1.5.18")
	testImplementation("org.assertj:assertj-core:3.27.3")
	testImplementation("org.junit.jupiter:junit-jupiter:5.12.1")
	testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

repositories {
    mavenLocal()
    mavenCentral()
    maven(url = "https://s01.oss.sonatype.org/content/repositories/snapshots/")
    google()
    maven(url = "https://jitpack.io")
}

version = System.getenv("VERSION") ?: "dev"

tasks {
    withType(Test::class) {
        useJUnitPlatform()
    }
    val shadowJar = withType(ShadowJar::class) {
        archiveClassifier = ""
        minimize()
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