package io.torvox.usb

import android.content.Context
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import io.mockk.every
import io.mockk.mockk
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Unit tests for [UsbSerialManager].
 *
 * The vendor-id → driver detection is pure logic and is fully exercised here.
 * Connection-state behaviour (no device attached) is verified with mocked
 * [Context]/[UsbManager] so no real USB hardware is required.
 */
class UsbSerialManagerTest {
    private fun mockDevice(
        vendorId: Int,
        productId: Int = 0,
        deviceClass: Int = 0,
    ): UsbDevice {
        val device = mockk<UsbDevice>(relaxed = true)
        every { device.vendorId } returns vendorId
        every { device.productId } returns productId
        every { device.deviceClass } returns deviceClass
        return device
    }

    private fun buildManager(): UsbSerialManager {
        val context = mockk<Context>(relaxed = true)
        val usbManager = mockk<UsbManager>(relaxed = true)
        every { context.getSystemService(Context.USB_SERVICE) } returns usbManager
        return UsbSerialManager(context)
    }

    @Test
    fun detectsFtdiByVendorId() {
        assertEquals(
            UsbSerialManager.DriverType.FTDI,
            UsbSerialManager.detectDriverType(mockDevice(vendorId = 0x0403)),
        )
    }

    @Test
    fun detectsCh340ByVendorId() {
        assertEquals(
            UsbSerialManager.DriverType.CH340,
            UsbSerialManager.detectDriverType(mockDevice(vendorId = 0x1A86)),
        )
    }

    @Test
    fun detectsCp210xByVendorId() {
        assertEquals(
            UsbSerialManager.DriverType.CP210X,
            UsbSerialManager.detectDriverType(mockDevice(vendorId = 0x10C4)),
        )
    }

    @Test
    fun detectsCdcAcmByCommClass() {
        assertEquals(
            UsbSerialManager.DriverType.CDC_ACM,
            UsbSerialManager.detectDriverType(
                mockDevice(vendorId = 0x1234, deviceClass = UsbConstants.USB_CLASS_COMM),
            ),
        )
    }

    @Test
    fun detectsCdcAcmByCdcDataClass() {
        assertEquals(
            UsbSerialManager.DriverType.CDC_ACM,
            UsbSerialManager.detectDriverType(
                mockDevice(vendorId = 0x1234, deviceClass = UsbConstants.USB_CLASS_CDC_DATA),
            ),
        )
    }

    @Test
    fun detectsUnknownForUnrecognisedDevice() {
        assertEquals(
            UsbSerialManager.DriverType.UNKNOWN,
            UsbSerialManager.detectDriverType(
                mockDevice(vendorId = 0x1234, deviceClass = UsbConstants.USB_CLASS_PER_INTERFACE),
            ),
        )
    }

    @Test
    fun driverTypeHasReadableDisplayName() {
        assertEquals("FTDI", UsbSerialManager.DriverType.FTDI.displayName)
        assertEquals("CH340/CH341", UsbSerialManager.DriverType.CH340.displayName)
    }

    @Test
    fun notConnectedBeforeAnyDevice() {
        val manager = buildManager()
        assertFalse(manager.isConnected())
        assertNull(manager.currentDevice())
    }

    @Test
    fun writeReturnsNegativeOneWhenNotConnected() {
        val manager = buildManager()
        assertEquals(-1, manager.write("hello".toByteArray()))
    }

    @Test
    fun writeLineDoesNotThrowWhenNotConnected() {
        // writeLine formats the payload and delegates to write, which returns
        // early (-1) when no device is attached. Verify it does not throw and
        // the manager stays consistent.
        val manager = buildManager()
        manager.writeLine("AT")
        assertFalse(manager.isConnected())
    }

    @Test
    fun closeWithoutDeviceDoesNotThrow() {
        val manager = buildManager()
        manager.close()
        assertFalse(manager.isConnected())
    }
}
