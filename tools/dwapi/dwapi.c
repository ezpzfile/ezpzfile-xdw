/* A small test tool around DocuWorks' own API (xdwapi.dll), run under Wine.
 *   dwapi info   FILE            document, pages, annotations and their settings
 *   dwapi make   FILE            add a date stamp, a sticky note (with text) and a text box to page 1, save
 *   dwapi render FILE PAGE OUT   page as an image file (DocuWorks draws it)
 * Strings are Shift_JIS (code page 932). */
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef void *HDOC;
typedef void *HANN;

typedef struct { int nSize, nOption, nAuthMode; } OPEN_MODE_EX;
typedef struct { int nSize, nPages, nVersion, nOriginalData, nDocType, nPermission, nShowAnnotations, nDocuments, nBinderColor, nBinderSize; } DOC_INFO;
typedef struct { int nSize, nWidth, nHeight, nPageType, nHorRes, nVerRes, nCompressType, nAnnotations; } PAGE_INFO;
typedef struct { int nSize; HANN handle; int nHorPos, nVerPos, nWidth, nHeight, nAnnotationType, nChildAnnotations; } ANN_INFO;
typedef struct { int nSize, nAnnotationType, nReserved1, nReserved2; } AA_INIT;
typedef struct { AA_INIT common; int nWidth, nHeight; } AA_WH;
typedef struct { AA_INIT common; int nWidth; } AA_W;
typedef struct { int nSize, nDpi, nColor; } IMAGE_OPTION;

typedef int (__stdcall *pOpen)(const char *, HDOC *, OPEN_MODE_EX *);
typedef int (__stdcall *pClose)(HDOC, void *);
typedef int (__stdcall *pSave)(HDOC, void *);
typedef int (__stdcall *pDocInfo)(HDOC, DOC_INFO *);
typedef int (__stdcall *pPageInfo)(HDOC, int, PAGE_INFO *);
typedef int (__stdcall *pAnnInfo)(HDOC, int, HANN, int, ANN_INFO *, void *);
typedef int (__stdcall *pGetAttr)(HANN, const char *, char *, int, void *);
typedef int (__stdcall *pAdd)(HDOC, int, int, int, int, void *, HANN *, void *);
typedef int (__stdcall *pAddOnParent)(HDOC, HANN, int, int, int, void *, HANN *, void *);
typedef int (__stdcall *pSetAttr)(HDOC, HANN, const char *, int, char *, int, void *);
typedef int (__stdcall *pConv)(HDOC, int, const char *, IMAGE_OPTION *);
typedef int (__stdcall *pDocName)(HDOC, int, char *, int, void *);
typedef int (__stdcall *pDocInfoBinder)(HDOC, int, DOC_INFO *, void *);

static HMODULE dll;
static void *fn(const char *name) {
    void *p = (void *)GetProcAddress(dll, name);
    if (!p) { fprintf(stderr, "missing %s\n", name); exit(2); }
    return p;
}

static const char *NAMES[] = {
    /* text */ "%Text", "%FontName", "%FontSize", "%FontStyle", "%ForeColor", "%BackColor", "%BorderColor", "%BorderStyle",
    /* shapes, sticky */ "%FillColor", "%BorderWidth", "%Transparent", "%AutoResize",
    /* stamp */ "%TopField", "%BottomField", "%DateStyle", "%YearField", "%MonthField", "%DayField", "%BasisYear",
    "%BasisYearStyle", "%DateFormat", "%DateOrder", "%DateFieldFirstChar",
    NULL };

static void attrs(pGetAttr get, HANN a, const char *pad) {
    for (int k = 0; NAMES[k]; k++) {
        char buf[4096];
        int n = get(a, NAMES[k], NULL, 0, NULL);
        if (n <= 0 || n > (int)sizeof buf) continue;
        memset(buf, 0, sizeof buf);
        if (get(a, NAMES[k], buf, n, NULL) < 0) continue;
        if (n == 4) printf("%s  %s = %d\n", pad, NAMES[k], *(int *)buf);
        else printf("%s  %s = \"%s\"\n", pad, NAMES[k], buf);
    }
}

static void walk(HDOC d, pAnnInfo ai, pGetAttr get, int page, HANN parent, int count, int depth) {
    char pad[32];
    memset(pad, ' ', sizeof pad);
    pad[depth * 2 + 2] = 0;
    for (int i = 1; i <= count; i++) {
        ANN_INFO a; memset(&a, 0, sizeof a); a.nSize = sizeof a;
        int r = ai(d, page, parent, i, &a, NULL);
        if (r < 0) { printf("%sannotation %d: error %08x\n", pad, i, r); continue; }
        printf("%sannotation %d type %d at %d,%d size %dx%d children %d\n", pad, i, a.nAnnotationType, a.nHorPos, a.nVerPos, a.nWidth, a.nHeight, a.nChildAnnotations);
        attrs(get, a.handle, pad);
        if (a.nChildAnnotations > 0) walk(d, ai, get, page, a.handle, a.nChildAnnotations, depth + 1);
    }
}

int main(int argc, char **argv) {
    if (argc < 3) { fprintf(stderr, "usage: dwapi info|make|render FILE ...\n"); return 1; }
    dll = LoadLibraryA("xdwapi.dll");
    if (!dll) { fprintf(stderr, "cannot load xdwapi.dll (%lu)\n", GetLastError()); return 2; }
    pOpen open = fn("XDW_OpenDocumentHandle");
    pClose close = fn("XDW_CloseDocumentHandle");
    OPEN_MODE_EX om = { sizeof om, (strcmp(argv[1], "make") == 0 || strcmp(argv[1], "stampvar") == 0) ? 1 : 0, 1 };
    HDOC d = NULL;
    int r = open(argv[2], &d, &om);
    if (r < 0) { printf("OPEN-ERROR %08x\n", r); return 3; }
    if (strcmp(argv[1], "info") == 0) {
        pDocInfo di = fn("XDW_GetDocumentInformation");
        pPageInfo pi = fn("XDW_GetPageInformation");
        pAnnInfo ai = fn("XDW_GetAnnotationInformation");
        pGetAttr get = fn("XDW_GetAnnotationAttribute");
        DOC_INFO info; memset(&info, 0, sizeof info); info.nSize = sizeof info;
        r = di(d, &info);
        printf("document pages %d version %d type %d documents %d (r=%d)\n", info.nPages, info.nVersion, info.nDocType, info.nDocuments, r);
        if (info.nDocuments > 0) {
            pDocName dn = fn("XDW_GetDocumentNameInBinder");
            pDocInfoBinder dib = fn("XDW_GetDocumentInformationInBinder");
            for (int k = 1; k <= info.nDocuments; k++) {
                char name[1024]; memset(name, 0, sizeof name);
                int n = dn(d, k, NULL, 0, NULL);
                if (n > 0 && n < (int)sizeof name) dn(d, k, name, n, NULL);
                DOC_INFO bi; memset(&bi, 0, sizeof bi); bi.nSize = sizeof bi;
                int rr = dib(d, k, &bi, NULL);
                printf("binder document %d \"%s\" pages %d (r=%d)\n", k, name, bi.nPages, rr);
            }
        }
        for (int p = 1; p <= info.nPages; p++) {
            PAGE_INFO pg; memset(&pg, 0, sizeof pg); pg.nSize = sizeof pg;
            r = pi(d, p, &pg);
            printf("page %d size %dx%d type %d annotations %d (r=%d)\n", p, pg.nWidth, pg.nHeight, pg.nPageType, pg.nAnnotations, r);
            walk(d, ai, get, p, NULL, pg.nAnnotations, 0);
        }
    } else if (strcmp(argv[1], "make") == 0) {
        pAdd add = fn("XDW_AddAnnotation");
        pAddOnParent addp = fn("XDW_AddAnnotationOnParentAnnotation");
        pSetAttr set = fn("XDW_SetAnnotationAttribute");
        pSave save = fn("XDW_SaveDocument");
        HANN a = NULL;
        /* date stamp */
        AA_W st; memset(&st, 0, sizeof st); st.common.nSize = sizeof st; st.common.nAnnotationType = 32819; st.nWidth = 1800;
        r = add(d, 32819, 1, 12000, 2000, &st, &a, NULL);
        printf("stamp add %d\n", r);
        if (r >= 0) {
            r = set(d, a, "%TopField", 1, "\x8e\xf3\x95\x74", 0, NULL); /* 受付 */
            printf("top %d\n", r);
            r = set(d, a, "%BottomField", 1, "\x8e\x52\x93\x63", 0, NULL); /* 山田 */
            printf("bottom %d\n", r);
            int manual = 1;
            r = set(d, a, "%DateStyle", 0, (char *)&manual, 0, NULL);
            printf("datestyle %d\n", r);
            r = set(d, a, "%YearField", 1, "26", 0, NULL); printf("year %d\n", r);
            r = set(d, a, "%MonthField", 1, "10", 0, NULL); printf("month %d\n", r);
            r = set(d, a, "%DayField", 1, "01", 0, NULL); printf("day %d\n", r);
        }
        /* sticky note with text on it */
        HANN f = NULL;
        AA_WH fs; memset(&fs, 0, sizeof fs); fs.common.nSize = sizeof fs; fs.common.nAnnotationType = 32794; fs.nWidth = 4000; fs.nHeight = 2500;
        r = add(d, 32794, 1, 2000, 2000, &fs, &f, NULL);
        printf("fusen add %d\n", r);
        if (r >= 0) {
            HANN t = NULL;
            AA_INIT ti; memset(&ti, 0, sizeof ti); ti.nSize = sizeof ti; ti.nAnnotationType = 32785;
            r = addp(d, f, 32785, 300, 300, NULL, &t, NULL);
            printf("fusen text add %d\n", r);
            if (r >= 0) { r = set(d, t, "%Text", 1, "\x8a\x6d\x94\x46\x82\xa8\x8a\xe8\x82\xa2", 0, NULL); printf("fusen text %d\n", r); } /* 確認お願い */
        }
        /* text box with background and frame */
        HANN tx = NULL;
        AA_INIT ti2; memset(&ti2, 0, sizeof ti2); ti2.nSize = sizeof ti2; ti2.nAnnotationType = 32785;
        r = add(d, 32785, 1, 2000, 6000, NULL, &tx, NULL);
        printf("text add %d\n", r);
        if (r >= 0) {
            r = set(d, tx, "%Text", 1, "TEXT BOX", 0, NULL); printf("text %d\n", r);
            int bg = 0x64FFFF; r = set(d, tx, "%BackColor", 0, (char *)&bg, 0, NULL); printf("back %d\n", r);
        }
        r = save(d, NULL);
        printf("save %d\n", r);
    } else if (strcmp(argv[1], "stampvar") == 0 && argc >= 7) {
        /* stampvar FILE YEAR FIRSTCHAR FORMAT COLOR(hex COLORREF) */
        pAdd add = fn("XDW_AddAnnotation");
        pSetAttr set = fn("XDW_SetAnnotationAttribute");
        pSave save = fn("XDW_SaveDocument");
        HANN a = NULL;
        AA_W st; memset(&st, 0, sizeof st); st.common.nSize = sizeof st; st.common.nAnnotationType = 32819; st.nWidth = 1800;
        r = add(d, 32819, 1, 15000, 2000, &st, &a, NULL); printf("add %d\n", r);
        int manual = 1, color = (int)strtol(argv[6], NULL, 16);
        printf("top %d\n", set(d, a, "%TopField", 1, "EZPZ", 0, NULL));
        printf("bottom %d\n", set(d, a, "%BottomField", 1, "TEST", 0, NULL));
        printf("style %d\n", set(d, a, "%DateStyle", 0, (char *)&manual, 0, NULL));
        printf("format %d\n", set(d, a, "%DateFormat", 1, argv[5], 0, NULL));
        printf("first %d\n", set(d, a, "%DateFieldFirstChar", 1, argv[4], 0, NULL));
        printf("year %d\n", set(d, a, "%YearField", 1, argv[3], 0, NULL));
        printf("month %d\n", set(d, a, "%MonthField", 1, "10", 0, NULL));
        printf("day %d\n", set(d, a, "%DayField", 1, "01", 0, NULL));
        if (argv[6][0] != '-') printf("color %d\n", set(d, a, "%BorderColor", 0, (char *)&color, 0, NULL));
        printf("save %d\n", save(d, NULL));
    } else if (strcmp(argv[1], "render") == 0 && argc >= 5) {
        pConv conv = fn("XDW_ConvertPageToImageFile");
        IMAGE_OPTION io = { sizeof io, atoi(argc > 5 ? argv[5] : "100"), 1 };
        r = conv(d, atoi(argv[3]), argv[4], &io);
        printf("render %d\n", r);
    }
    close(d, NULL);
    return 0;
}
