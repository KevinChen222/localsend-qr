LocalSend QR 0.2.2 — Windows x64 / Ubuntu 24 x64

中文说明
========
这是基于 LocalSend Rust 核心的独立便携工具，无需安装 LocalSend。
解压到可长期保留的目录。手机无需 App，扫码后用系统浏览器发送或下载。

Windows 11 x64
1. 双击 Register-ContextMenu.cmd 注册右键，无需修改 PowerShell 执行策略。
2. 右键一个文件或文件夹 → 显示更多选项 → 由二维码发送。
3. 终端显示二维码；多网卡时先选择手机可以连接到的 Wi-Fi/以太网地址。
4. 手机扫码，点击“下载文件”。请保持终端窗口开启。
   手机发到电脑：右键目标文件夹或文件夹空白处 → 由二维码接收。
   手机扫码后选择文件，点击“发送到电脑”，文件保存在该文件夹。
5. 关闭分享用 Ctrl+C。卸载右键双击 Remove-ContextMenu.cmd。
6. 移动便携包后重新运行 Register-ContextMenu.cmd，更新调用路径。
也可以在终端执行 .\localsend-qr.exe context-menu install 注册，或
.\localsend-qr.exe context-menu remove 移除菜单。
可用 context-menu install --core-path 'D:\其他目录\localsend-qr.exe' 指定核心位置。
原 .ps1 注册/移除入口保留供允许 PowerShell 脚本的环境使用。
也可直接使用 EXE 分享文件，无需注册任何右键菜单。

Ubuntu 24 x64（默认 GNOME Files / Nautilus）
1. 在解压目录运行 bash register-context-menu.sh。
2. 右键选中的文件/文件夹 → 脚本 → 由二维码发送。
   手机发到电脑：选中目标文件夹 → 脚本 → 由二维码接收。
   本地当前目录也可从脚本菜单启动；不选文件时使用当前目录。
3. 移动便携包后重跑注册脚本；可给脚本传入核心程序绝对路径。
4. 移除菜单运行 bash remove-context-menu.sh。
Nautilus 菜单需要 gnome-terminal；命令行分享不需要桌面环境。
在 NTFS/exFAT 等不保留执行权限的盘上，建议先解压到 Ubuntu 用户目录。
Nautilus 通过换行传递选中路径，含换行的文件名请改用命令行。

剪贴板与终端
============
发送、接收窗口就绪后自动复制完整内网网址（含本次访问凭证），
手机同步剪贴板后直接粘贴到浏览器即可打开。窗口只显示二维码和一行状态；
接收计数原地更新。需要显示网址时加 --print-url。
Windows 使用系统剪贴板，无需额外程序。
Linux Wayland 需要 wl-clipboard（wl-copy），X11 需要 xclip 或 xsel。
例如 Ubuntu 可按桌面环境安装 sudo apt install wl-clipboard 或 sudo apt install xclip。
没有图形剪贴板或复制失败时显示“剪贴板不可用”，仍可扫码或用 --print-url。

命令行
======
Windows: .\localsend-qr.exe share -- 'D:\中文 文件.txt'
Ubuntu:  ./localsend-qr share -- '/home/user/中文 文件.txt'
接收:    ./localsend-qr receive -- '/home/user/接收目录'
Windows: .\localsend-qr.exe receive -- 'D:\接收目录'
当前目录: ./localsend-qr receive
网址:    ./localsend-qr receive --print-url -- DIRECTORY
多项:    ./localsend-qr share -- FILE1 FILE2 FOLDER
端口:    ./localsend-qr share --port 0 -- FILE  （自动选择空闲端口）
网卡:    ./localsend-qr share --ip 192.168.1.20 -- FILE
时限:    ./localsend-qr share --expires 0 -- FILE （不自动过期）
配置:    EXE/核心程序旁的 settings.json；命令行参数优先。
首选端口 53318；被系统保留或其他窗口占用时自动选择空闲端口。
二维码和剪贴板始终使用实际端口，支持同时打开多个传输窗口。默认分享 60 分钟。
默认时限到达会终止进行中的传输；大文件可提高时限或用 --expires 0。

行为与网络
==========
单文件直接传输，不复制整个文件到内存。文件夹或多项先生成一个未压缩 ZIP，
保留子目录和空目录，支持 ZIP64；需要系统临时目录有足够空间。
正常停止会删除临时 ZIP；强制结束进程或断电可能留下系统临时文件。
符号链接和特殊文件会报错，避免意外把其他目录内容一起分享。
Windows 右键第一版支持单个文件或文件夹；多选请用命令行。
接收支持手机多文件发送、进度显示、取消及重复发送。同名文件自动添加 (1)、(2) 等后缀。
接收先写目标目录内的临时 .part 文件，完成并检查字节数后才保存为正式文件；
取消或中断会清理未完成的临时文件，已完成文件保留。强制结束或断电可能留下 .part。
接收文件名按 Windows/Linux 共同限制校验：不接受路径、系统保留名称、特殊字符、
末尾句点/空格及超过 200 UTF-8 字节的名称。接收目录须已存在且可写。
二维码使用每次随机生成的访问凭证。得到完整链接的人可以下载所选内容或向接收目录上传。
传输使用 HTTP，未加密，请在可信局域网使用。文件不经云端。
手机和电脑必须可以互相访问；访客 Wi-Fi/AP 隔离、VPN 和防火墙可能阻断。
Windows 第一次可能弹出防火墙提示；允许专用网络。必要时自行以管理员运行
Allow-PrivateNetwork.ps1，仅允许专用网络中本子网设备访问这个程序。
此规则在移动程序后需重跑；不用时可在防火墙中移除 LocalSendQR-Portable-Private。
Ubuntu 若启用了 UFW，可手动只对局域网网段开放使用的 TCP 端口。
浏览器决定保存位置，照片不会自动进入相册；微信内置浏览器可能需要转到系统浏览器。
发送模式的电脑终端不显示下载完成提示；请以手机浏览器的保存结果为准。

许可与来源
==========
项目: https://github.com/KevinChen222/localsend-qr
上游: https://github.com/localsend/localsend
Apache-2.0，保留上游 LICENSE。此 fork 为独立扩展，不是官方发行版。
