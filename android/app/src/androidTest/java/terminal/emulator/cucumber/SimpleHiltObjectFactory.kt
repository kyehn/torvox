package terminal.emulator.cucumber

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

    private fun <T> createInstance(classLiteral: Class<T>): T {
        val injectConstructor = findInjectConstructor(classLiteral)
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
            return classLiteral.getDeclaredConstructor().newInstance()
        } catch (exception: NoSuchMethodException) {
            throw RuntimeException(
                "Cannot create instance of ${classLiteral.name}: no @Inject constructor or no-arg constructor",
            )
        }
    }

    @Suppress("UNCHECKED_CAST")
    private fun <T> findInjectConstructor(classLiteral: Class<T>): Constructor<T>? {
        for (constructor in classLiteral.declaredConstructors) {
            if (constructor.isAnnotationPresent(Inject::class.java)) {
                return constructor as Constructor<T>
            }
        }
        return null
    }
}
