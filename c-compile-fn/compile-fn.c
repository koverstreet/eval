#include <dlfcn.h>
#include <errno.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>

int foo1(void)
{
	return 5;
}

static void *__compile_fn(const char *env_str,
			  const char *ret_str,
			  const char *args_str,
			  const char *body_str)
{
	struct stat statbuf;
	FILE *f;

	char template[100];
	strcpy(template, "tmp-compiled-fn-XXXXXX");

	char *dir = mkdtemp(template);
	if (!dir) {
		perror("error creating tmpdir:");
		exit(EXIT_FAILURE);
	}

	char srcfile[200], objfile[200], binfile[200], cmd[200];
	snprintf(srcfile, sizeof(srcfile), "%s/compiled_fn.c", dir);
	snprintf(objfile, sizeof(objfile), "%s/compiled_fn.o", dir);
	snprintf(binfile, sizeof(binfile), "%s/compiled_fn.bin", dir);

	/* create source file: */
	f = fopen(srcfile, "w+");
	if (!f) {
		fprintf(stderr, "error opening %s: %s\n", srcfile, strerror(errno));
		exit(EXIT_FAILURE);
	}
	fprintf(f, "%s\n%s compiled_fn%s { %s }",
		env_str, ret_str, args_str, body_str);
	fclose(f);

	/* build: */
	snprintf(cmd, sizeof(cmd), "gcc -c -o %s %s", objfile, srcfile);
	if (system(cmd)) {
		printf("error compiling\n");
		exit(EXIT_FAILURE);
	}

	/* allocate output buffer (hopefully won't grow when linked..): */
	if (stat(objfile, &statbuf)) {
		perror("error statting output");
		exit(EXIT_FAILURE);
	}

	void *fn = mmap(NULL, statbuf.st_size,
			PROT_EXEC|PROT_READ|PROT_WRITE,
			MAP_PRIVATE|MAP_ANONYMOUS|MAP_32BIT, 0, 0);

	/* build linker script: */
	char linker_script[200];
	snprintf(linker_script, sizeof(linker_script), "%s/linker.ld", dir);

	f = fopen(linker_script, "w+");
	if (!f) {
		fprintf(stderr, "error opening %s: %s\n", linker_script, strerror(errno));
		exit(EXIT_FAILURE);
	}

	fprintf(f,
		"ENTRY(compiled_fn)\n"
		"SECTIONS {\n"
			". = %p;\n"
			".text : { *(.text) }\n"
			".data : { *(.data) }\n"
			".bss : { *(.bss) }\n"
		"}\n",
		fn);

	/* resolve symbols: */
	snprintf(cmd, sizeof(cmd), "nm -u  %s", objfile);
	void *self = dlopen(NULL, RTLD_LAZY);
	if (!self) {
		fprintf(stderr, "dlopen error: %s\n", dlerror());
		exit(EXIT_FAILURE);
	}

	char *line = NULL;
	size_t n = 0;
	ssize_t len;

	FILE *nm_out = popen(cmd, "r");
	while ((len = getline(&line, &n, nm_out)) != -1) {
		if (len <= 20)
			continue;

		while (strlen(line) && line[strlen(line) - 1] == '\n')
			line[strlen(line) - 1] = '\0';

		const char *sym = line + 19;
		void *addr = dlsym(self, sym);
		char *error = dlerror();

		if (!addr) {
			fprintf(stderr, "error resolving %s: %s (len %zu)\n",
				sym, error, strlen(sym));
			exit(EXIT_FAILURE);
		}

		fprintf(f, "PROVIDE_HIDDEN(%s = %p);\n", sym, addr);
	}
	fclose(nm_out);
	dlclose(self);
	fclose(f);

	/* link: */
	snprintf(cmd, sizeof(cmd), "ld -T %s --oformat binary -o %s %s",
		 linker_script, binfile, objfile);
	if (system(cmd)) {
		printf("error linking\n");
		exit(EXIT_FAILURE);
	}

	/* read final output: */
	f = fopen(binfile, "r");
	if (!f) {
		fprintf(stderr, "error opening %s: %s\n", binfile, strerror(errno));
		exit(EXIT_FAILURE);
	}

	if (fstat(fileno(f), &statbuf)) {
		perror("error statting output");
		exit(EXIT_FAILURE);
	}
	if (fread(fn, statbuf.st_size, 1, f) != 1) {
		perror("fread error");
		exit(EXIT_FAILURE);
	}
	fclose(f);

	return fn;
}

#define compile_fn(_env, _ret, _args, _body)				\
({									\
	_ret (*_fn)_args = __compile_fn(_env, #_ret, #_args, #_body);	\
	_fn;								\
})

int main(int argc, char **argv)
{
	/*
	printf("test1(3, 5) = %i\n",
	       compile_fn("", int, (int x, int y), { return x * y; })(3, 5));
	       */

	printf("test2(5) = %i\n",
	       compile_fn("int foo1(void);",
			  int, (int x), {return x * foo1();})(5));
	exit(EXIT_SUCCESS);
}
