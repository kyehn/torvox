package terminal.emulator.benchmark

import android.os.BatteryManager
import android.os.PowerManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 设备环境快照的解析判据。
 *
 * 样本取自模拟器上的真实输出（API 35 模拟器，`getprop`、`dumpsys battery`、
 * `dumpsys thermalservice`、`dumpsys display`、`dumpsys package`）：断言用字面量而非
 * 重新计算的期望值，格式漂移时即判红。`getprop`、`dumpsys display` 与 `dumpsys package`
 * 的样本按相关行裁剪（原始输出分别约 100、510 与 164 行），裁剪处标注行首；包 dump 里每次
 * 安装都会变的包标识哈希已去掉（与解析无关，且长得像提交号）。
 */
class DeviceEnvironmentTest {
    @Test
    fun parsesSystemProperties() {
        val properties = parseSystemProperties(SYSTEM_PROPERTIES_SAMPLE)

        assertEquals("sdk_gphone64_x86_64", properties["ro.product.model"])
        assertEquals(
            "sdk_gphone64_x86_64-userdebug 15 AE3A.240806.043 12960925 dev-keys",
            properties["ro.build.display.id"],
        )
        assertEquals("35", properties["ro.build.version.sdk"])
    }

    @Test
    fun parsesBatteryState() {
        val battery = parseBatteryState(BATTERY_OUTPUT_SAMPLE)

        assertEquals(100, battery.levelPercent)
        assertEquals(BatteryManager.BATTERY_STATUS_CHARGING, battery.status)
        // dump 里的温度单位是 0.1 摄氏度。
        assertEquals(25.0, battery.temperatureCelsius, 0.0)
    }

    @Test
    fun parsesThermalStatus() {
        assertEquals(PowerManager.THERMAL_STATUS_NONE, parseThermalStatus(THERMAL_OUTPUT_SAMPLE))
    }

    @Test
    fun parsesRefreshRateHz() {
        assertEquals(60.000004, parseRefreshRateHz(DISPLAY_OUTPUT_SAMPLE), 0.0)
    }

    @Test
    fun fallsBackToDisplayDeviceRefreshRate() {
        assertEquals(60.000004, parseRefreshRateHz(DISPLAY_DEVICE_INFO_ONLY_SAMPLE), 0.0)
    }

    @Test
    fun parsesPackageVersion() {
        val version = parsePackageVersion(PACKAGE_OUTPUT_SAMPLE)

        assertEquals(2000L, version.versionCode)
        assertEquals("0.1.0", version.versionName)
    }

    @Test
    fun describesEnvironmentForMeasuredLine() {
        val environment = DeviceEnvironment(
            model = "sdk_gphone64_x86_64",
            romBuildId = "sdk_gphone64_x86_64-userdebug 15 AE3A.240806.043 12960925 dev-keys",
            apiLevel = 35,
            refreshRateHz = 60.000004,
            batteryLevelPercent = 100,
            batteryStatus = BatteryManager.BATTERY_STATUS_CHARGING,
            batteryTemperatureCelsius = 25.0,
            thermalStatus = PowerManager.THERMAL_STATUS_NONE,
            animationScales = AnimationScales("1.0", "1.0", "1.0"),
            fixedPerformanceMode = FixedPerformanceState.UNVERIFIABLE,
            capturedAtEpochMillis = 1_795_279_264_131L,
        )

        val description = environment.describe()
        assertTrue(description.contains("model=sdk_gphone64_x86_64"))
        assertTrue(description.contains("api=35"))
        assertTrue(description.contains("refreshRate=60.000004Hz"))
        assertTrue(description.contains("batteryStatus=CHARGING"))
        assertTrue(description.contains("thermal=NONE"))
        assertTrue(description.contains("animationScales=window=1.0"))
        assertTrue(description.contains("fixedPerformanceMode=UNVERIFIABLE"))
    }

    @Test
    fun parsesAppIdentityFromPackageDump() {
        val identity = parseAppIdentity(PACKAGE_OUTPUT_SAMPLE, "com.termux")

        assertEquals("com.termux", identity.packageName)
        assertEquals(2000L, identity.versionCode)
        assertEquals("0.1.0", identity.versionName)
        assertTrue(identity.describe().contains("versionCode=2000"))
    }

    @Test
    fun rejectsBatteryDumpWithoutLevel() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseBatteryState("  status: 2\n  temperature: 250\n")
        }

        assertTrue(failure.message.orEmpty().contains("level"))
    }

    @Test
    fun rejectsDisplayDumpWithoutRefreshRate() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parseRefreshRateHz("  Display Devices: size=1\n")
        }

        assertTrue(failure.message.orEmpty().contains("刷新率"))
    }

    @Test
    fun rejectsPackageDumpWithoutVersionName() {
        val failure = assertThrows(IllegalStateException::class.java) {
            parsePackageVersion("    versionCode=2000 minSdk=31 targetSdk=28\n")
        }

        assertTrue(failure.message.orEmpty().contains("versionName"))
    }

    private companion object {
        /** 真实 `getprop` 输出（裁剪到本仓关心的属性）。 */
        val SYSTEM_PROPERTIES_SAMPLE =
            """
            [ro.build.display.id]: [sdk_gphone64_x86_64-userdebug 15 AE3A.240806.043 12960925 dev-keys]
            [ro.build.version.sdk]: [35]
            [ro.product.model]: [sdk_gphone64_x86_64]
            """.trimIndent()

        /** 真实 `dumpsys battery` 输出。 */
        val BATTERY_OUTPUT_SAMPLE =
            """
            Current Battery Service state:
              AC powered: true
              USB powered: false
              Wireless powered: false
              Dock powered: false
              Max charging current: 5000000
              Max charging voltage: 5000000
              Charge counter: 10000
              status: 2
              health: 2
              present: true
              level: 100
              scale: 100
              voltage: 5000
              temperature: 250
              technology: Li-ion
              Charging state: 0
              Charging policy: 0
            """.trimIndent()

        /** 真实 `dumpsys thermalservice` 输出。 */
        val THERMAL_OUTPUT_SAMPLE =
            """
            IsStatusOverride: false
            ThermalEventListeners:
              callbacks: 5
              killed: false
              broadcasts count: -1
            ThermalStatusListeners:
              callbacks: 3
              killed: false
              broadcasts count: -1
            Thermal Status: 0
            Cached temperatures:
              Temperature{mValue=30.8, mType=3, mName=test temperature sensor, mStatus=0}
            HAL Ready: true
            HAL connection:
              ThermalHAL 2.0 connected: yes
            Current temperatures from HAL:
              Temperature{mValue=30.8, mType=3, mName=test temperature sensor, mStatus=0}
            Current cooling devices from HAL:
              CoolingDevice{mValue=100, mType=0, mName=test cooling device}
            Temperature static thresholds from HAL:
              TemperatureThreshold{mType=3, mName=test temperature sensor, mHotThrottlingThresholds=[NaN, NaN, NaN, 48.0, NaN, NaN, 60.0], mColdThrottlingThresholds=[NaN, NaN, NaN, NaN, NaN, NaN, NaN]}
            Temperature headroom thresholds:
              [NaN, NaN, NaN, NaN, NaN, NaN, 1.4]
            """.trimIndent()

        /** 真实 `dumpsys display` 输出（裁剪到刷新率相关的两行）。 */
        val DISPLAY_OUTPUT_SAMPLE =
            """
              DisplayDeviceInfo{"Built-in Screen": uniqueId="local:4619827259835644672", 320 x 640, modeId 1, renderFrameRate 60.000004, defaultModeId 1, userPreferredModeId -1, supportedModes [{id=1, width=320, height=640, fps=60.000004, vsync=60.000004, synthetic=false, alternativeRefreshRates=[], supportedHdrTypes=[]}], colorMode 0, supportedColorModes [0], hdrCapabilities HdrCapabilities{mSupportedHdrTypes=[], mMaxLuminance=500.0, mMaxAverageLuminance=500.0, mMinLuminance=0.0}, allmSupported false, gameContentTypeSupported false, density 160, 160.0 x 160.0 dpi, appVsyncOff 1000000, presDeadline 16666666, touch INTERNAL, rotation 0, type INTERNAL, address {port=0, model=0x401cec6a7a2b7b}, deviceProductInfo DeviceProductInfo{name=EMU_display_0, manufacturerPnpId=GGL, productId=1, modelYear=null, manufactureDate=ManufactureDate{week=27, year=2006}, connectionToSinkType=0}, state ON, committedState ON, frameRateOverride , brightnessMinimum 0.0, brightnessMaximum 1.0, brightnessDefault 0.39763778, hdrSdrRatio NaN, FLAG_ALLOWED_TO_BE_DEFAULT_DISPLAY, FLAG_ROTATES_WITH_CONTENT, FLAG_SECURE, FLAG_SUPPORTS_PROTECTED_BUFFERS, FLAG_TRUSTED, installOrientation 0, displayShape DisplayShape{ spec=-2047665221 displayWidth=320 displayHeight=640 physicalPixelDisplaySizeRatio=1.0 rotation=0 offsetX=0 offsetY=0 scale=1.0}}
                mActiveRenderFrameRate=60.000004
            """.trimIndent()

        /** 只有显示设备信息、没有活动帧率时的真实行（兜底路径）。 */
        val DISPLAY_DEVICE_INFO_ONLY_SAMPLE =
            """
              DisplayDeviceInfo{"Built-in Screen": uniqueId="local:4619827259835644672", 320 x 640, modeId 1, renderFrameRate 60.000004, defaultModeId 1, userPreferredModeId -1}
            """.trimIndent()

        /** 真实 `dumpsys package com.termux` 输出（裁剪到版本标识两行）。 */
        val PACKAGE_OUTPUT_SAMPLE =
            """
              Package [com.termux]:
                appId=10223
                codePath=/data/app/~~XtjRPnc39rV0BAWPc6_amQ==/com.termux-Ip5OpiVkpZJ9wAkYdJBW2A==
                versionCode=2000 minSdk=31 targetSdk=28
                minExtensionVersions=[]
                versionName=0.1.0
            """.trimIndent()
    }
}
