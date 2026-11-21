# 一加 Ace 6（`ossi` / PLQ110）Skyward 移植

本仓库是 [kirklin/skyward](https://github.com/kirklin/skyward) 针对**一加 Ace 6（PLQ110）**的移植分支，
仓库名 `skyward-for-oneplusace6`。它不是一加或上游 Skyward 的官方发布。

- 设备内部代号：`ossi`（`ro.product.device`），平台 `sun`（SM8750）
- 测试系统：ColorOS **V17.0.0** / Android 17
- 测试槽位：`a`，Bootloader 已解锁
- 基线提交：上游 `c5e09f6`

## 这个分支与上游的关系

`upstream` 指向上游 `kirklin/skyward`，本分支在上游 `c5e09f6` 之上有 9 个提交：

```
7ebf365 fix(bootimg): Windows 构建（解包 stub + lz4 回退）
388b98a feat(ossi): 设备配置 + 本文档
da4d946 fix: 模块目录列表 + rooted 路径
bcab6a6 fix(usb): 停止反复枚举
88f8758 fix(info): build.prop 根目录
238d49e fix(modules): 两种布局 + 跨目录依赖
cad76c1 feat(input): 触屏中断
16e7d49 fix(ui): OPPO 亮度属性
3b9fecc feat(ossi): 基础移植（基于 Ral-sei/skyward-for-oneplusace5）
c5e09f6 ← 上游基线
```

其中**前六个是通用修复**，对其它 OPLUS 机型同样适用（见文末「可向上游提交的部分」）。
本仓库**不包含**移植时使用的 Windows 构建辅助工具（打包器 `ktm-pack` 与 `build-ktm.ps1`），
只保留源码、设备配置与文档；构建方式见下文，用上游的标准流程即可。

## 当前状态

已实测可用：

| 功能 | 说明 |
| --- | --- |
| 显示 | 1272×2800 @165Hz，按 DRM mode 字符串 `1272x2800x165cmd` 配置 |
| 亮度 | OPPO 面板用 `oplus,dsi-bl-normal-max-level`（3515/4094） |
| **触屏** | Synaptics S3910，走 OPLUS hbp 层，需使能中断（见下文） |
| 音量键导航 | 上游自带，可用 |
| adb | root，无需手机端授权 |
| 分区挂载 | super 映射 19 个逻辑分区 + modem + persist |
| **data 解密** | FBEv2 + wrappedkey_v0，密码正确后 `/data` 与 `/sdcard` 可读写 |
| **Wi-Fi** | `qca_cld3_kiwi_v2`，可扫描、连接 |
| 机型显示 | OnePlus Ace 6 / 骁龙 8 至尊版 |
| OS 版本显示 | V17.0.0 |

未验证：OTA 安装、双系统、MTP 文件传输的读写细节。

## 刷入

只需要 `recovery` 分区，**不需要改 vendor_boot 或 boot**。

```sh
fastboot flash recovery skyward-ossi.img
fastboot reboot recovery
```

回滚：

```sh
fastboot flash recovery <原厂 recovery.img>
```

**注意**：镜像的 AVB footer 是未签名的，解锁状态下才能启动，**不要重新锁 BL**。

## 这个移植解决的七个问题

按发现顺序，每个都附了根因和证据。

### 1. 黑屏：亮度属性只认小米的

OPPO 面板的亮度上限在设备树里叫 `oplus,dsi-bl-normal-max-level`，而上游只找小米的
`mi,max-brightness-normal` 等三个名字，找不到就 `bail`，recovery 起不来。

实测这块面板（`qcom,mdss_dsi_panel_AA605_P_7_A0020_dsc_cmd`）：

```
oplus,dsi-bl-normal-max-level       3515
qcom,mdss-dsi-bl-max-level          4094
```

**修法**：`crates/skyward/src/ui.rs` 的 `NORMAL_MAX` 表加一行：

```rust
("oplus,dsi-bl-normal-max-level", "qcom,mdss-dsi-bl-max-level"),
```

### 2. 密钥链不启动：`[[owners]]` 等一个不存在的节点

原 `ossi.toml` 里有一条 `[[owners]]` 等 `/dev/nq-nci`。这台机器的 NFC 走 **TMS** 方案，
内核建的是 `/dev/tms_nfc`，`/dev/nq-nci` 永远不出现。`hand_over` 等 5 秒后 `bail`，
**整个 `bring_up` 中止**，后面的密钥服务一个都不启动。

证据：

```
skyward: devices not brought up: wait for a file the modules just loaded
         make: /dev/nq-nci is not there after 5000 ms
```

**修法**：删掉那条 `[[owners]]`。这台机器没有 weaver，密钥只走 TEE，不需要 eSE 节点。

### 3. 密钥服务反复重启：缺 `/odm/lib64`

```
CANNOT LINK EXECUTABLE "./vendor.qti.hardware.qseecom@1.0-service":
  library "vendor.qti.hardware.qseecom@1.0-impl.so" not found
```

这个 `-impl.so` 在 `/odm/lib64`。A/B 对照确认：

| 库路径 | 结果 |
| --- | --- |
| `/system/lib64:/vendor/lib64` | 退出，`-impl.so not found` |
| 加上 `/vendor/lib64/hw:/odm/lib64` | 存活 |

**修法**：四个密钥服务的 `libraries` 补上这两个路径。

### 4. 触屏无响应：TP 中断默认关闭（本移植最关键的一条）

OPLUS 的 hbp 触屏层把中断关着，等用户态来打开。原厂 recovery 由 `touchDaemon` +
`hbp5-service` 负责，但它们在 recovery 环境里起不来（缺 Android 服务），于是中断永远
不触发，`/dev/input/event6` 一个事件都没有：

```
326: 1 0 0 0 0 0 0 0  msmgpio 162 Level  touch-00     ← 只有 probe 那一次
```

**修法**来自 OPLUS 开源内核 `vendor/oplus/kernel/tp/hbp/hbp/hbp_device.c`：

```c
#define HBP_IOCTRL_GROUP      0xC5
#define HBP_IOCTRL_IRQ_ENABLE _IO(HBP_IOCTRL_GROUP, 0x0A)   // = 0xC50A

case HBP_IOCTRL_IRQ_ENABLE:
    hbp_set_irq_status(hbp_dev, usr.val);
```

对 `/dev/hbp0` 调一次这个 ioctl 即可，**中断一旦打开就永久有效**（进程退出、fd 关闭后
仍工作），所以只需开机调用一次。

代码在 `crates/skyward/src/input.rs` 的 `wake_touch_interrupt()`，由 `Inputs::open()`
开头调用。

**关键细节**：参数是 `union usr_data`，**第一个成员就是 `int64_t val`（偏移 0）**。
写成 `{cmd, val}` 会让内核读到 0，静默失败。实测内核日志：

```
HBP_IOCTRL_IRQ_ENABLE:0    ← 结构体错
HBP_IOCTRL_IRQ_ENABLE:1    ← 正确
```

打开 `/dev/hbp_core` 会让面板进入 hbp 模式（数据走帧队列、不走 evdev），而 `/dev/hbp0`
不会，所以只碰后者。

### 5. Wi-Fi 打不开：四个叠加的问题

**(a) `rfkill.ko` 不在 skyward 找的路径**

```
skyward: the WLAN did not come up: rfkill.ko is in none of
["/lib/modules", "/vendor_dlkm/lib/modules", "/vendor/lib/modules",
 "/system_dlkm/flatten/lib/modules"]
```

实际在 `/system_dlkm/lib/modules/`。上游只认 AOSP 的 `flatten/` 布局。

**修法**：`crates/skyward-modules/src/lib.rs` 的 `system_dlkm` 改成两种布局都试。

**(b) 模块依赖跨分区，上游只在同目录找**

`cfg80211.ko`（vendor）依赖 `rfkill.ko`（system_dlkm）。上游拿 vendor 的
`modules.dep` 解析，找不到文件就放弃：

```
rfkill.ko is listed in /vendor_dlkm/lib/modules/modules.dep but missing
```

**修法**：`load()` 里每个依赖在所有配置目录里搜索，而不是只看自己那个目录。

**(c) 配置里漏了 `cfg80211`**

`kiwi_v2` 依赖 `cfg80211`，不显式加载会报 `Unknown symbol cfg80211_*`。

**(d) modem 没启动**

`cnss2` 需要 modem 的 QMI 服务，否则一直重试：

```
cnss: DMS QMI connection not established
```

**修法**：加一条 `[[service]]` 启动 `remoteproc1`（mss/modem）和 `remoteproc0`（adsp）。

四个都修完后的加载链（实测日志）：

```
skyward: loaded /system_dlkm/lib/modules/rfkill.ko
skyward: loaded /lib/modules/cfg80211.ko
skyward: loaded /vendor_dlkm/lib/modules/qca_cld3_kiwi_v2.ko
```

然后 `wlan0` 与 `p2p0` 出现。

### 6. OS 版本显示 AOSP 构建号

About 页的版本读自已安装系统的 `build.prop`，而 ColorOS 把
`ro.build.version.oplusrom=V17.0.0` 放在**分区根目录**的 `build.prop`（`/my_product/build.prop`），
上游只读 `etc/build.prop` 和 `system/system/build.prop`。

**修法**：`crates/skyward-info/src/lib.rs` 的路径映射补上根目录的 `build.prop`。

### 7. USB 周期性重新枚举

`mtp.rc` 里 `mount functionfs` 带了 `no_disconnect=1`：adbd 关闭时 functionfs 不主动断开，
Windows 看到一个「有设备但不响应」的端口，于是重新枚举。另外启动路径用 `switch("adb")`
会先经过 `none`，而 `none` 触发 `mtp.rc` 删掉 adb 的函数链接。

**修法**：

- 去掉 `no_disconnect=1`
- `main.rs` 启动时改用 `set("adb")` 直接请求，不经过 `none`

## 与其它一加移植的差异

| 机型 | 触屏 | 需要改 vendor_boot？ |
| --- | --- | --- |
| 一加 13（sun） | 内核直接报 evdev | 否 |
| 一加 Ace 2 Pro（PJA110） | 改 vendor_boot 模块加载顺序 | **是** |
| **一加 Ace 6（本移植）** | **hbp ioctl 使能中断** | **否** |

这台机器不能走 PJA110 的路：`oplus_hbp_core` 和 `oplus_bsp_tp_hbp_syna_s3910` 都标了
`[permanent]`，卸不掉；设备树兼容名是 `synaptics,tcm-spi-hbp`，普通 SPI 驱动绑不上。

## 构建

需要 Rust 1.100.0、Android NDK r29、cargo-ndk：

```sh
export ANDROID_NDK_HOME=/path/to/android-ndk-r29

cargo ndk -t arm64-v8a --platform 30 build --release -p skyward

RUSTFLAGS="-C target-feature=+crt-static" \
  cargo ndk -t arm64-v8a --platform 30 build --release \
  -p skyward-system-init --target-dir target/system-init

cargo build --release -p skyward-pack

target/release/skyward-pack recovery \
  --stock recovery_stock.img \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/ossi.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --out skyward-ossi.img
```

`--stock` 是原厂 `recovery` 分区的整镜像（100 MiB，带 AVB footer）。

## 校验

```
f981fd154742a4bcac2abc17ecde5ebe7ce0f50d8e119a55b05617a58caa7f7a  skyward-ossi.img
```

## 说明

- 只应刷 `recovery` 分区，不要刷 `boot` / `init_boot` / `vendor_boot`
- recovery 下 adb 是 root 且不需要手机端授权，注意数据安全
- Wi-Fi 从点开到能扫描需要十几秒：modem 固件加载 + cnss 校准是硬件固有时间
- 本仓库基于 GPL-3.0-or-later 的 Skyward，遵循同一许可证

## 可向上游提交的部分

下面六个提交不依赖这台设备，对其它机型同样成立，可以考虑拆成独立 PR 提到上游：

| 提交 | 影响面 | 说明 |
| --- | --- | --- |
| `16e7d49` | 所有 OPLUS 机型 | 面板亮度属性 `oplus,dsi-bl-normal-max-level` |
| `cad76c1` | 所有走 hbp 触屏的 OPLUS 机型 | 开机使能触屏中断 |
| `238d49e` | 所有 GKI 机型 | `system_dlkm` 两种布局 + 跨目录模块依赖 |
| `88f8758` | 所有 OPLUS 机型 | 分区根目录的 `build.prop` |
| `da4d946` | 通用 | 每个模块源的所有候选目录 |
| `7ebf365` | 非 Unix 构建主机 | 解包 stub + lz4 回退 |

`bcab6a6`（USB 重新枚举）改的是 `mtp.rc` 与启动路径，**可能影响其它机型的 USB 行为**，
建议单独提，并附上本机的实测现象。

`388b98a` 与 `3b9fecc` 是设备专属内容，不宜混进通用 PR。

