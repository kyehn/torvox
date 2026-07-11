package io.torvox.usb

import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import android.util.Log
import java.io.Closeable

/**
 * USB serial communication manager.
 *
 * Supports CDC-ACM, FTDI, and CH340/CH341 USB-to-serial adapters commonly used
 * with embedded devices, routers, and microcontrollers.
 *
 * Referenced from Haven's USB subsystem and Termux's USB serial support.
 */
class UsbSerialManager(
    private val context: Context,
) : Closeable {
    data class SerialDevice(
        val device: UsbDevice,
        val name: String,
        val vendorId: Int,
        val productId: Int,
        val driverType: DriverType,
    )

    enum class DriverType(
        val displayName: String,
    ) {
        CDC_ACM("CDC-ACM"),
        FTDI("FTDI"),
        CH340("CH340/CH341"),
        CP210X("CP210x"),
        UNKNOWN("Unknown"),
    }

    interface SerialListener {
        fun onDeviceAttached(device: SerialDevice)

        fun onDeviceDetached(device: SerialDevice)

        fun onDataReceived(data: ByteArray)

        fun onError(error: String)
    }

    private val usbManager: UsbManager = context.getSystemService(Context.USB_SERVICE) as UsbManager
    private var connection: UsbDeviceConnection? = null
    private var readEndpoint: UsbEndpoint? = null
    private var writeEndpoint: UsbEndpoint? = null
    private var readThread: Thread? = null
    private var isReading = false
    private var listener: SerialListener? = null
    private var currentDevice: SerialDevice? = null

    private val usbReceiver =
        object : BroadcastReceiver() {
            override fun onReceive(
                context: Context,
                intent: Intent,
            ) {
                when (intent.action) {
                    ACTION_USB_PERMISSION -> {
                        val device: UsbDevice? =
                            intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
                                ?: return
                        val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                        if (granted) {
                            device?.let { connectToDevice(it) }
                                ?: Log.w(TAG, "USB device is null despite permission grant")
                        } else {
                            listener?.onError("USB permission denied")
                        }
                    }

                    UsbManager.ACTION_USB_DEVICE_DETACHED -> {
                        val device: UsbDevice? = intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
                        if (device != null && device.deviceId == currentDevice?.device?.deviceId) {
                            disconnect()
                        }
                    }
                }
            }
        }

    fun setListener(listener: SerialListener?) {
        this.listener = listener
    }

    fun register() {
        val filter =
            IntentFilter().apply {
                addAction(ACTION_USB_PERMISSION)
                addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            }
        context.registerReceiver(usbReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
    }

    fun unregister() {
        try {
            context.unregisterReceiver(usbReceiver)
        } catch (exception: IllegalArgumentException) {
            Log.w(TAG, "USB receiver not registered", exception)
        }
    }

    fun listDevices(): List<SerialDevice> {
        val devices = mutableListOf<SerialDevice>()
        for (device in usbManager.deviceList.values) {
            val driverType = detectDriverType(device)
            if (driverType != DriverType.UNKNOWN || device.deviceClass == UsbConstants.USB_CLASS_CDC_DATA) {
                val serial =
                    SerialDevice(
                        device = device,
                        name = device.productName ?: device.deviceName,
                        vendorId = device.vendorId,
                        productId = device.productId,
                        driverType = driverType,
                    )
                devices.add(serial)
            }
        }
        return devices
    }

    @SuppressLint("MutableImplicitPendingIntent")
    fun requestPermission(device: UsbDevice) {
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
        val permissionIntent =
            PendingIntent.getBroadcast(
                context,
                0,
                Intent(ACTION_USB_PERMISSION),
                flags,
            )
        usbManager.requestPermission(device, permissionIntent)
    }

    private fun connectToDevice(device: UsbDevice) {
        val usbInterface =
            findSerialInterface(device) ?: run {
                listener?.onError("No serial interface found on ${device.deviceName}")
                return
            }

        val deviceConnection =
            usbManager.openDevice(device) ?: run {
                listener?.onError("Failed to open USB device ${device.deviceName}")
                return
            }

        if (!deviceConnection.claimInterface(usbInterface, true)) {
            deviceConnection.close()
            listener?.onError("Failed to claim USB interface")
            return
        }

        val (readEp, writeEp) =
            findEndpoints(usbInterface) ?: run {
                deviceConnection.releaseInterface(usbInterface)
                deviceConnection.close()
                listener?.onError("No suitable endpoints found")
                return
            }

        connection = deviceConnection
        readEndpoint = readEp
        writeEndpoint = writeEp

        val driverType = detectDriverType(device)
        val serialDevice =
            SerialDevice(
                device = device,
                name = device.productName ?: device.deviceName,
                vendorId = device.vendorId,
                productId = device.productId,
                driverType = driverType,
            )
        currentDevice = serialDevice
        listener?.onDeviceAttached(serialDevice)

        startReading()
    }

    fun disconnect() {
        stopReading()
        releaseConnection()
        connection = null
        readEndpoint = null
        writeEndpoint = null
        currentDevice?.let { listener?.onDeviceDetached(it) }
        currentDevice = null
    }

    private fun releaseConnection() {
        val deviceConnection = connection ?: return
        val usbInterface = currentDevice?.device?.let { findSerialInterface(it) }
        if (usbInterface != null) {
            try {
                deviceConnection.releaseInterface(usbInterface)
            } catch (exception: Exception) {
                Log.w(TAG, "Failed to release USB interface", exception)
            }
        }
        deviceConnection.close()
    }

    fun write(data: ByteArray): Int {
        val deviceConnection = connection ?: return -1
        val endpoint = writeEndpoint ?: return -1
        return deviceConnection.bulkTransfer(endpoint, data, data.size, WRITE_TIMEOUT_MS)
    }

    fun writeLine(text: String) {
        val bytes = (text + "\r\n").toByteArray()
        write(bytes)
    }

    private fun startReading() {
        isReading = true
        readThread =
            Thread({
                val buffer = ByteArray(READ_BUFFER_SIZE)
                while (isReading) {
                    val deviceConnection = connection ?: break
                    val endpoint = readEndpoint ?: break
                    val len = deviceConnection.bulkTransfer(endpoint, buffer, buffer.size, READ_TIMEOUT_MS)
                    if (len > 0) {
                        val data = buffer.copyOf(len)
                        listener?.onDataReceived(data)
                    } else if (len < 0) {
                        Thread.sleep(RETRY_SLEEP_MS)
                    }
                }
            }, "usb-serial-reader").apply {
                isDaemon = true
                start()
            }
    }

    private fun stopReading() {
        isReading = false
        readThread?.join(500)
        readThread = null
    }

    private fun findSerialInterface(device: UsbDevice): UsbInterface? {
        for (i in 0 until device.interfaceCount) {
            val usbInterface = device.getInterface(i)
            if (usbInterface.interfaceClass == UsbConstants.USB_CLASS_CDC_DATA ||
                usbInterface.interfaceClass == UsbConstants.USB_CLASS_COMM ||
                usbInterface.interfaceClass == UsbConstants.USB_CLASS_VENDOR_SPEC
            ) {
                return usbInterface
            }
        }
        return null
    }

    private fun findEndpoints(usbInterface: UsbInterface): Pair<UsbEndpoint, UsbEndpoint>? {
        var readEp: UsbEndpoint? = null
        var writeEp: UsbEndpoint? = null
        for (i in 0 until usbInterface.endpointCount) {
            val endpoint = usbInterface.getEndpoint(i)
            if (endpoint.type == UsbConstants.USB_ENDPOINT_XFER_BULK) {
                if (endpoint.direction == UsbConstants.USB_DIR_IN) {
                    readEp = endpoint
                } else {
                    writeEp = endpoint
                }
            }
        }
        return if (readEp != null && writeEp != null) Pair(readEp, writeEp) else null
    }

    fun isConnected(): Boolean = connection != null

    fun currentDevice(): SerialDevice? = currentDevice

    override fun close() {
        disconnect()
        unregister()
    }

    companion object {
        private const val TAG = "UsbSerialManager"
        private const val ACTION_USB_PERMISSION = "io.torvox.USB_PERMISSION"
        private const val READ_BUFFER_SIZE = 4096
        private const val READ_TIMEOUT_MS = 1000
        private const val WRITE_TIMEOUT_MS = 1000
        private const val RETRY_SLEEP_MS = 10L

        private val FTDI_VENDOR_IDS = setOf(0x0403)
        private val CH340_VENDOR_IDS = setOf(0x1A86)
        private val CP210X_VENDOR_IDS = setOf(0x10C4)

        fun detectDriverType(device: UsbDevice): DriverType = when (device.vendorId) {
            in FTDI_VENDOR_IDS -> {
                DriverType.FTDI
            }

            in CH340_VENDOR_IDS -> {
                DriverType.CH340
            }

            in CP210X_VENDOR_IDS -> {
                DriverType.CP210X
            }

            else -> {
                if (device.deviceClass == UsbConstants.USB_CLASS_CDC_DATA ||
                    device.deviceClass == UsbConstants.USB_CLASS_COMM
                ) {
                    DriverType.CDC_ACM
                } else {
                    DriverType.UNKNOWN
                }
            }
        }
    }
}
