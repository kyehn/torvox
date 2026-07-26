package io.term.concurrency

import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.ConcurrentModificationException
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

class ConcurrentSessionMapTest {
    @Test
    fun snapshotIterationDoesNotThrowCME() {
        val map = ConcurrentHashMap<Long, String>()
        for (i in 0L until 100L) {
            map[i] = "session-$i"
        }
        val executor = Executors.newFixedThreadPool(4)
        val errors = java.util.Collections.synchronizedList(mutableListOf<Throwable>())

        val rounds = 100
        val latches = Array(rounds) { CountDownLatch(2) }

        for (round in 0 until rounds) {
            val latch = latches[round]
            executor.submit {
                try {
                    for (i in 0 until 500) {
                        map.keys.sorted()
                        map.keys.toList()
                    }
                } catch (exception: ConcurrentModificationException) {
                    errors.add(exception)
                } finally {
                    latch.countDown()
                }
            }
            executor.submit {
                try {
                    for (i in 0 until 500) {
                        val key = Thread.currentThread().threadId() * 100000L + i
                        map[key] = "session-$i"
                        if (i % 3 == 0) map.remove(key - 1)
                    }
                } catch (exception: ConcurrentModificationException) {
                    errors.add(exception)
                } finally {
                    latch.countDown()
                }
            }
        }

        for (latch in latches) {
            latch.await(10, TimeUnit.SECONDS)
        }

        executor.shutdown()
        executor.awaitTermination(5, TimeUnit.SECONDS)

        assertTrue(
            "ConcurrentModificationException thrown during snapshot iteration: ${errors.joinToString("\n")}",
            errors.isEmpty(),
        )
    }
}
