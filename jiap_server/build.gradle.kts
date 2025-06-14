plugins {
	kotlin("jvm") version "1.9.25"
	kotlin("plugin.spring") version "1.9.25"
	id("org.springframework.boot") version "3.4.4"
	id("io.spring.dependency-management") version "1.1.7"
}

group = "me.yvesz"

val jiapServerVersion by extra { System.getenv("JADX_SERVER_VERSION") ?: "dev" }
println("jiap_server version: $jiapServerVersion")
version = jiapServerVersion

val jadxBuildJavaVersion by extra { getBuildJavaVersion() }
val jadxSdkVersion = "1.5.2"

fun getBuildJavaVersion(): Int? {
	val envVarName = "JADX_BUILD_JAVA_VERSION"
	val buildJavaVer = System.getenv(envVarName)?.toInt() ?: return null
	if (buildJavaVer < 11) {
		throw GradleException("'$envVarName' can't be set to lower than 11")
	}
	println("Set Java toolchain for jadx build to version '$buildJavaVer'")
	return buildJavaVer
}

java {
	jadxBuildJavaVersion?.let { buildJavaVer ->
		toolchain {
			languageVersion = JavaLanguageVersion.of(buildJavaVer)
		}
	}
}

repositories {
	mavenCentral()
	google()
}

dependencies {
	implementation("org.springframework.boot:spring-boot-starter-web")
	implementation("org.springframework.boot:spring-boot-starter-websocket")
	implementation("com.fasterxml.jackson.module:jackson-module-kotlin")
	implementation("org.springframework.boot:spring-boot-starter-thymeleaf")
	implementation("org.jetbrains.kotlin:kotlin-reflect")
	implementation("com.google.code.gson:gson:2.10.1")

	implementation("io.github.skylot:jadx-core:$jadxSdkVersion")
	implementation("io.github.skylot:jadx-cli:$jadxSdkVersion")
	implementation("io.github.skylot:jadx-rename-mappings:$jadxSdkVersion")
	implementation("io.github.skylot:jadx-dex-input:$jadxSdkVersion")
	implementation("io.github.skylot:jadx-smali-input:$jadxSdkVersion")
	implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.5.1")

	testImplementation("org.springframework.boot:spring-boot-starter-test")
	testImplementation("org.jetbrains.kotlin:kotlin-test-junit5")
	testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

kotlin {
	compilerOptions {
		freeCompilerArgs.addAll("-Xjsr305=strict")
	}
}

tasks.withType<Test> {
	useJUnitPlatform()
}

tasks.register<Copy>("dist") {
	dependsOn(tasks.bootJar)
	from(tasks.bootJar.get().outputs.files)
	into(layout.projectDirectory.dir("out"))
}

tasks.register<Delete>("cleanOutDir") {
	delete(layout.projectDirectory.dir("out"))
}
