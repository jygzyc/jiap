#!/bin/bash
export JAR_PATH="out/jiap_server-dev.jar"

java -jar $JAR_PATH --spring.config.location=application.properties --server.port=8080 