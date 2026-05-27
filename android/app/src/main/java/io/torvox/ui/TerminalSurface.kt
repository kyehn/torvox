package io.torvox.ui

import android.content.Context
import android.util.AttributeSet
import android.view.GestureDetector
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import io.torvox.SelectionMode
import io.torvox.TerminalViewModel

class TerminalSurface
    @JvmOverloads
    constructor(
        context: Context,
        attrs: AttributeSet? = null,
        defStyleAttr: Int = 0,
    ) : SurfaceView(context, attrs, defStyleAttr),
        SurfaceHolder.Callback {
        private var viewModel: TerminalViewModel? = null
        private var rows: Int = 24
        private var cols: Int = 80
        private var isScrolling: Boolean = false
        private var scrollOffset: Int = 0
        private var maxScrollOffset: Int = 0

        var onScrollChanged: ((offset: Int) -> Unit)? = null
        var onScrollingStateChanged: ((isScrolling: Boolean) -> Unit)? = null

        private val gestureListener =
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onDown(e: MotionEvent): Boolean = true

                override fun onScroll(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    distanceX: Float,
                    distanceY: Float,
                ): Boolean {
                    if (!isScrolling) {
                        isScrolling = true
                        onScrollingStateChanged?.invoke(true)
                    }
                    val scrollAmount = (distanceY / 16f).toInt().coerceAtLeast(1)
                    val newOffset = (scrollOffset + scrollAmount).coerceIn(0, maxScrollOffset)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                    }
                    return true
                }

                override fun onFling(
                    e1: MotionEvent?,
                    e2: MotionEvent,
                    velocityX: Float,
                    velocityY: Float,
                ): Boolean {
                    val flingAmount = (velocityY / 100f).toInt().coerceIn(-50, 50)
                    val newOffset = (scrollOffset + flingAmount).coerceIn(0, maxScrollOffset)
                    if (newOffset != scrollOffset) {
                        scrollOffset = newOffset
                        onScrollChanged?.invoke(scrollOffset)
                    }
                    postDelayed({
                        isScrolling = false
                        onScrollingStateChanged?.invoke(false)
                    }, 300)
                    return true
                }

                override fun onSingleTapUp(e: MotionEvent): Boolean {
                    if (isScrolling) {
                        isScrolling = false
                        scrollOffset = 0
                        onScrollChanged?.invoke(0)
                        onScrollingStateChanged?.invoke(false)
                        return true
                    }
                    viewModel?.clearSelection()
                    return false
                }

                override fun onLongPress(e: MotionEvent) {
                    val row = (e.y / 16f).toInt().coerceIn(0, rows - 1)
                    val col = (e.x / 8f).toInt().coerceIn(0, cols - 1)
                    viewModel?.startSelection(row, col)
                }
            }

        private val gestureDetector = GestureDetector(context, gestureListener)

        init {
            holder.addCallback(this)
            isFocusable = true
            isFocusableInTouchMode = true
        }

        fun initialize(viewModel: TerminalViewModel) {
            this.viewModel = viewModel
        }

        fun setDimensions(
            rows: Int,
            cols: Int,
        ) {
            this.rows = rows
            this.cols = cols
        }

        fun setMaxScrollback(maxLines: Int) {
            this.maxScrollOffset = maxLines
        }

        fun setScrollOffset(offset: Int) {
            this.scrollOffset = offset.coerceIn(0, maxScrollOffset)
            onScrollChanged?.invoke(this.scrollOffset)
        }

        fun getScrollOffset(): Int = scrollOffset

        fun isCurrentlyScrolling(): Boolean = isScrolling

        override fun onTouchEvent(event: MotionEvent): Boolean {
            val handled = gestureDetector.onTouchEvent(event)
            if (event.actionMasked == MotionEvent.ACTION_MOVE && viewModel
                    ?.state
                    ?.value
                    ?.selection
                    ?.active == true
            ) {
                val row = (event.y / 16f).toInt().coerceIn(0, rows - 1)
                val col = (event.x / 8f).toInt().coerceIn(0, cols - 1)
                viewModel?.updateSelection(row, col)
            }
            if (event.actionMasked == MotionEvent.ACTION_UP || event.actionMasked == MotionEvent.ACTION_CANCEL) {
                viewModel?.endSelection()
            }
            return handled || super.onTouchEvent(event)
        }

        override fun surfaceCreated(holder: SurfaceHolder) {}

        override fun surfaceChanged(
            holder: SurfaceHolder,
            format: Int,
            width: Int,
            height: Int,
        ) {}

        override fun surfaceDestroyed(holder: SurfaceHolder) {
            isScrolling = false
            scrollOffset = 0
        }

        override fun onDetachedFromWindow() {
            super.onDetachedFromWindow()
            handler.removeCallbacksAndMessages(null)
        }
    }
