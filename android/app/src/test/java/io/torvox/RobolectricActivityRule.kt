package io.torvox

import androidx.activity.ComponentActivity
import org.junit.rules.TestRule
import org.junit.runner.Description
import org.junit.runners.model.Statement
import org.robolectric.Robolectric
import org.robolectric.android.controller.ActivityController

class RobolectricActivityRule<A : ComponentActivity>(
    private val activityClass: Class<A>,
) : TestRule {
    lateinit var activity: A
        private set

    lateinit var controller: ActivityController<A>
        private set

    override fun apply(
        base: Statement,
        description: Description,
    ): Statement = object : Statement() {
        override fun evaluate() {
            controller = Robolectric.buildActivity(activityClass)
            activity =
                controller
                    .create()
                    .start()
                    .resume()
                    .visible()
                    .get()
            try {
                base.evaluate()
            } finally {
                controller.pause().stop().destroy()
            }
        }
    }
}
