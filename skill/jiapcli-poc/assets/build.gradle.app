plugins {
    id 'com.android.application'
}

android {
    namespace 'com.poc.<target_app>'
    compileSdk 34

    defaultConfig {
        applicationId "com.poc.<target_app>"
        minSdk 26
        targetSdk 34
        versionCode 1
        versionName "1.0"
    }

    buildTypes {
        release {
            minifyEnabled false
        }
    }
    compileOptions {
        sourceCompatibility JavaVersion.VERSION_1_8
        targetCompatibility JavaVersion.VERSION_1_8
    }

    dependenciesInfo {
        includeInApk = false
        includeInBundle = false
    }
}

repositories {
    mavenCentral()
}

dependencies {
    implementation 'androidx.appcompat:appcompat:1.6.1'
    implementation 'org.lsposed.hiddenapibypass:hiddenapibypass:4.0'
}
