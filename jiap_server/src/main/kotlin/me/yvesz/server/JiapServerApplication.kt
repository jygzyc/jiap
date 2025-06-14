package me.yvesz.server

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication

@SpringBootApplication
class JiapServerApplication

fun main(args: Array<String>) {
	runApplication<JiapServerApplication>(*args)
}