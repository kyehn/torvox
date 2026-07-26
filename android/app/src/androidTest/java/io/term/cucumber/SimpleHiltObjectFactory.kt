package io.term.cucumber

import io.cucumber.core.backend.ObjectFactory
import java.lang.reflect.Constructor
import javax.inject.Inject

class SimpleHiltObjectFactory : ObjectFactory {
    private val instances = HashMap<Class<*>, Any?>()

    override fun start() {
    }

    override fun stop() {
        instances.clear()
    }

    override fun addClass(glueClass: Class<*>): Boolean = true

    override fun <T> getInstance(glueClass: Class<T>): T {
        @Suppress("UNCHECKED_CAST")
        return instances.getOrPut(glueClass) {
            createInstance(glueClass)
        } as T
    }

    private fun <T> createInstance(cls: Class<T>): T {
        val injectConstructor = findInjectConstructor(cls)
        if (injectConstructor != null) {
            val paramInstances =
                injectConstructor.parameterTypes
                    .map {
                        instances[it] ?: createInstance(it)
                    }.toTypedArray()
            @Suppress("UNCHECKED_CAST")
            return injectConstructor.newInstance(*paramInstances) as T
        }
        try {
            return cls.getDeclaredConstructor().newInstance()
        } catch (e: NoSuchMethodException) {
            throw RuntimeException(
                "Cannot create instance of ${cls.name}: no @Inject constructor or no-arg constructor",
            )
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun <T> findInjectConstructor(cls: Class<T>): Constructor<T>? {
        for (constructor in cls.declaredConstructors) {
            if (constructor.isAnnotationPresent(Inject::class.java)) {
                return constructor as Constructor<T>
            }
        }
        return null
    }
}
