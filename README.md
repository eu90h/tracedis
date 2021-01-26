An Introduction to libtrace.so and tracedis
===========================================
While working on an UEFI application, I needed to debug an obscure problem caused by hooking some functions during the OS bootup phase. While I had success using gdb, I wished that I had a simple trace of all the instructions executed on my virtual machine used for testing. The solution I ended up devising was to write a QEMU plugin that logs the bytes of all executed instructions to a file, which is then parsed by another tool I wrote called tracedis which takes the trace file produced by the QEMU plugin and disassembles it into a human-readable format.

One of the unique challenges presented here is that, since I'm working with an x86_64 machine, the disassembler has to understand which sections of the trace are in 16 bit mode, 32 bit mode, and 64 bit mode.

libtrace.so
-----------
The library [libtrace.so](https://github.com/eu90h/qemu/tree/plugin-libtrace) is a simple QEMU plugin whose job is simply to write the bytes corresponding to executed instructions into a file. QEMU plugins are a relatively new feature, so for this experiment I built QEMU from source with the --enable-plugins configure flag set. At this point, documentation on QEMU plugins seems to be spare, but the API looks simple enough. A handful of example/test plugins are provided which form a good foundation for further development. Let's take a look at trace.c, which is the entirety of libtrace.so:

	/*
	 * Copyright (C) 2020, euler90h@gmail.com
	 *
	 * Based on insn.c by Emilio G. Cota <cota@braap.org>.
	 *
	 * License: GNU GPL, version 2 or later.
	 *   See the COPYING file in the top-level directory.
	 */
	#include <inttypes.h>
	#include <assert.h>
	#include <fcntl.h>
	#include <stdlib.h>
	#include <string.h>
	#include <unistd.h>
	#include <stdio.h>
	#include <glib.h>

	#include <qemu-plugin.h>

	QEMU_PLUGIN_EXPORT int qemu_plugin_version = QEMU_PLUGIN_VERSION;

	static int log_fd;
	static char default_output_path[] = "./trace.bin";
	static const char *outputh_path = default_output_path;

	static void vcpu_tb_trans(qemu_plugin_id_t id, struct qemu_plugin_tb *tb)
	{
	    size_t n = qemu_plugin_tb_n_insns(tb);
	    size_t i;

	    for (i = 0; i < n; i++) {
	        struct qemu_plugin_insn *insn = qemu_plugin_tb_get_insn(tb, i);
	        const unsigned char *data = qemu_plugin_insn_data(insn);
	        size_t len = qemu_plugin_insn_size(insn);
	        ssize_t bytes_written = write(log_fd, data, len);
	        assert(bytes_written != -1);
	    }
	}
	                
	static void plugin_exit(qemu_plugin_id_t id, void *p)
	{
	    close(log_fd);
	}

	QEMU_PLUGIN_EXPORT int qemu_plugin_install(qemu_plugin_id_t id,
	                                           const qemu_info_t *info,
	                                           int argc, char **argv)
	{
	    if (argc && argv != NULL && argv[0] != NULL) {
	        outputh_path = argv[0];
	    }

	    log_fd = open(outputh_path, O_WRONLY | O_CREAT, S_IRUSR | S_IWUSR);
	    assert(log_fd != -1);
	    qemu_plugin_register_vcpu_tb_trans_cb(id, vcpu_tb_trans);
	    qemu_plugin_register_atexit_cb(id, plugin_exit, NULL);
	    return 0;
	}

The entry point is qemu_plugin_install. This simple function checks for a path argument and sets the output_path global if appropriate before attempting to open the log file for writing, creating the log if it doesn't already exist. Next, callbacks for translation and exit are set. The interesting part is the translation callback vcpu_tb_trans. This function is given a block of instructions which are iterated through while writing their bytes to the log file. Finally, at the exit point plugin_exit is called which merely closes the log file descriptor.

tracedis
--------
tracedis (short for trace disassembler) is a simple Rust program whose job is to disassemble the stream of instructions within the trace file. The source of tracedis isn't particularly interesting, so I'll just show an example usage: `./tracedis windows_boot.trace 57 A1337`
This invocation means disassemble the windows_boot.trace file whose 16bit region ends at offset 0x57 and whose 32bit region ends at 0xA1337, and whose 64bit region makes up the remainder of the trace.

Example Usage
-------------
Assume we have a QEMU disk set up containing a Windows 10 installation that boots in MBR (i.e. non-UEFI) mode. We wish to capture the trace from machine start up to the login screen. We start QEMU with the tracing plugin enabled: `qemu-system-x86_64 -d plugin -plugin path/to/libtrace.so,arg=output_log_path -drive ...`
When we wish to end tracing, we simply exit QEMU.

After QEMU exits, we will have a trace log which can be read by tracedis. Parse the trace file like so: `tracedis trace.bin 47 a683c`. You will get output that resembles something like

	BEGINNING 16-BIT REGION
	0000000000000000 90                   nop
	0000000000000001 90                   nop
	0000000000000002 EB94                 jmp 0FF98h
	0000000000000004 BF4250               mov di,5042h
	0000000000000007 EB0A                 jmp 13h
	0000000000000009 EBF9                 jmp 4h
	000000000000000B 6689C4               mov esp,eax
	000000000000000E EB02                 jmp 12h
	0000000000000010 EB85                 jmp 0FF97h
	0000000000000012 FA                   cli
	0000000000000013 BB00F0               mov bx,0F000h
	0000000000000016 8EDB                 mov ds,bx
	0000000000000018 BB58FF               mov bx,0FF58h
	000000000000001B 2E660F0117           lgdt cs:[bx]
	0000000000000020 66B823000040         mov eax,40000023h
	0000000000000026 0F22C0               mov cr0,eax
	0000000000000029 66EA3FFFFFFF1000     jmp 10h:0FFFFFF3Fh
	0000000000000031 B84006               mov ax,640h
	0000000000000034 0000                 add [bx+si],al
	0000000000000036 0F22E0               mov cr4,eax
	0000000000000039 66B808008ED8         mov eax,0D88E0008h
	000000000000003F 8EC0                 mov es,ax
	0000000000000041 8EE0                 mov fs,ax
	0000000000000043 8EE8                 mov gs,ax
	0000000000000045 8ED0                 mov ss,ax
	...
	...
	...

We see here the classic prelude to entering 32bit mode. Keep in mind that the numbers on the left are not memory addresses but rather offsets within the trace file.
