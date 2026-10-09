/* New text on a page: a pen, lines, content (PDF-03). */

#include <math.h>
#include <string.h>

#include "ely_shim.h"

typedef enum
{
	PEN_CID,
	PEN_SIMPLE,
	PEN_CJK
} pen_kind;

/* A font to write with and its resource. */
typedef struct
{
	pen_kind kind;
	fz_font *font;
	int ordering;
	pdf_obj *ref;
} pen;

/* What to write, where, in what. */
typedef struct
{
	const char *text;
	const char *font;
	float size;
	int rgb;
	/* Page space: left, top, width. */
	float box[3];
} text_args;

/* The script face for typed signatures, from the worker. */
static const unsigned char *script_data;
static size_t script_len;

void ely_script_font(const unsigned char *data, size_t len)
{
	script_data = data;
	script_len = len;
}

/* The pen named `choice`; "original" writes in `own`. */
static void choose_pen(fz_context *ctx, const char *choice, fz_font *own, pen *p)
{
	static const struct { const char *choice, *name; } latin[] = {
		{ "helvetica", "Helvetica" }, { "times", "Times-Roman" }, { "courier", "Courier" } };
	static const struct { const char *choice; int ordering; } cjk[] = {
		{ "cjk-sc", FZ_ADOBE_GB }, { "cjk-tc", FZ_ADOBE_CNS }, { "cjk-jp", FZ_ADOBE_JAPAN }, { "cjk-kr", FZ_ADOBE_KOREA } };
	size_t i;
	if (!strcmp(choice, "script"))
	{
		if (!script_data)
			fz_throw(ctx, FZ_ERROR_ARGUMENT, "no script font was given");
		p->kind = PEN_CID;
		p->font = fz_new_font_from_memory(ctx, "GreatVibes", script_data, (int)script_len, 0, 0);
		return;
	}
	if (!strcmp(choice, "original"))
	{
		if (!own || !pdf_font_writing_supported(ctx, own))
			fz_throw(ctx, FZ_ERROR_ARGUMENT, "the document's font cannot write new text");
		p->kind = PEN_CID;
		p->font = fz_keep_font(ctx, own);
		return;
	}
	for (i = 0; i < nelem(latin); i++)
		if (!strcmp(choice, latin[i].choice))
		{
			p->kind = PEN_SIMPLE;
			p->font = fz_new_base14_font(ctx, latin[i].name);
			return;
		}
	for (i = 0; i < nelem(cjk); i++)
		if (!strcmp(choice, cjk[i].choice))
		{
			p->kind = PEN_CJK;
			p->ordering = cjk[i].ordering;
			p->font = fz_new_cjk_font(ctx, cjk[i].ordering);
			return;
		}
	fz_throw(ctx, FZ_ERROR_ARGUMENT, "no font %s", choice);
}

/* The pen's font as a resource of `pdf`. */
static void add_pen(fz_context *ctx, pdf_document *pdf, pen *p)
{
	switch (p->kind)
	{
	case PEN_CID:
		p->ref = pdf_add_cid_font(ctx, pdf, p->font);
		break;
	case PEN_SIMPLE:
		p->ref = pdf_add_simple_font(ctx, pdf, p->font, PDF_SIMPLE_ENCODING_LATIN);
		break;
	case PEN_CJK:
		p->ref = pdf_add_cjk_font(ctx, pdf, p->font, p->ordering, 0, 0);
		break;
	}
}

/* The pen's glyph for `u`; 0 when it has none. */
static int glyph(fz_context *ctx, pen *p, int u)
{
	if (p->kind == PEN_SIMPLE && fz_windows_1252_from_unicode(u) < 0)
		return 0;
	return fz_encode_character(ctx, p->font, u);
}

/* `u` as the pen's code, in hex. */
static void code(fz_context *ctx, fz_buffer *buf, pen *p, int u, int gid)
{
	switch (p->kind)
	{
	case PEN_CID:
		fz_append_printf(ctx, buf, "%04x", gid);
		break;
	case PEN_SIMPLE:
		fz_append_printf(ctx, buf, "%02x", fz_windows_1252_from_unicode(u));
		break;
	case PEN_CJK:
		if (u > 0xffff)
			fz_append_printf(ctx, buf, "%04x%04x", 0xd800 + ((u - 0x10000) >> 10), 0xdc00 + ((u - 0x10000) & 0x3ff));
		else
			fz_append_printf(ctx, buf, "%04x", u);
		break;
	}
}

/* `text` as code points; the count is returned. */
static int runes(fz_context *ctx, const char *text, int **out)
{
	int n = 0, c;
	const char *s;
	for (s = text; *s; n++)
		s += fz_chartorune(&c, s);
	*out = fz_malloc_array(ctx, n + 1, int);
	for (n = 0, s = text; *s; n++)
		s += fz_chartorune(&(*out)[n], s);
	return n;
}

static float advance(fz_context *ctx, pen *p, int u, float size)
{
	return fz_advance_glyph(ctx, p->font, glyph(ctx, p, u), 0) * size;
}

/* Lines no wider than `width`: [from, to) each. */
static int lay_out(fz_context *ctx, pen *p, const int *u, int n, float size, float width, int *from, int *to,
	float *widest)
{
	int i = 0, lines = 0;
	*widest = 0;
	while (i <= n)
	{
		int j, space = -1;
		float run = 0, before = 0;
		for (j = i; j < n && u[j] != '\n'; j++)
		{
			float w = advance(ctx, p, u[j], size);
			if (run + w > width && j > i)
				break;
			if (u[j] == ' ')
				space = j, before = run;
			run += w;
		}
		from[lines] = i;
		if (j < n && u[j] != '\n' && space > i)
		{
			/* Too long for the line: wrap at a space. */
			to[lines++] = space;
			*widest = fz_max(*widest, before);
			i = space + 1;
		}
		else
		{
			to[lines++] = j;
			*widest = fz_max(*widest, run);
			i = j < n && u[j] != '\n' ? j : j + 1;
		}
	}
	return lines;
}

/* A resource name in `type` that is free, given `ref`. */
const char *ely_add_resource(fz_context *ctx, pdf_document *pdf, pdf_obj *page, pdf_obj *type, const char *stem,
	pdf_obj *ref, char *name, size_t room)
{
	pdf_obj *res = pdf_dict_get_inheritable(ctx, page, PDF_NAME(Resources));
	pdf_obj *dict;
	int n;
	if (!res)
		res = pdf_dict_put_dict(ctx, page, PDF_NAME(Resources), 2);
	dict = pdf_dict_get(ctx, res, type);
	if (!dict)
		dict = pdf_dict_put_dict(ctx, res, type, 2);
	for (n = 1;; n++)
	{
		fz_snprintf(name, room, "%s%d", stem, n);
		if (!pdf_dict_gets(ctx, dict, name))
			break;
	}
	pdf_dict_puts(ctx, dict, name, ref);
	return name;
}

/* The marker of a page whose own content Ely wrapped. */
static const char wrap[] = "q\n%Ely\n";

/* Adds `buf` after the page's content; the new stream. */
pdf_obj *ely_append_content(fz_context *ctx, pdf_document *pdf, pdf_obj *page, fz_buffer *buf)
{
	pdf_obj *contents = pdf_dict_get(ctx, page, PDF_NAME(Contents));
	pdf_obj *list, *first = pdf_is_array(ctx, contents) ? pdf_array_get(ctx, contents, 0) : NULL;
	fz_buffer *head = NULL, *mark = first ? pdf_load_stream(ctx, first) : NULL;
	pdf_obj *added = NULL;
	int wrapped;
	fz_var(head);
	fz_try(ctx)
	{
		wrapped = mark && mark->len == strlen(wrap) && !memcmp(mark->data, wrap, mark->len);
		if (wrapped)
			list = contents;
		else
		{
			list = pdf_new_array(ctx, pdf, 4);
			head = fz_new_buffer_from_copied_data(ctx, (const unsigned char *)wrap, strlen(wrap));
			pdf_array_push_drop(ctx, list, pdf_add_stream(ctx, pdf, head, NULL, 0));
			if (pdf_is_array(ctx, contents))
			{
				int i;
				for (i = 0; i < pdf_array_len(ctx, contents); i++)
					pdf_array_push(ctx, list, pdf_array_get(ctx, contents, i));
			}
			else if (contents)
				pdf_array_push(ctx, list, contents);
			fz_drop_buffer(ctx, head);
			head = fz_new_buffer_from_copied_data(ctx, (const unsigned char *)"Q\n", 2);
			pdf_array_push_drop(ctx, list, pdf_add_stream(ctx, pdf, head, NULL, 0));
			pdf_dict_put_drop(ctx, page, PDF_NAME(Contents), list);
		}
		added = pdf_add_stream(ctx, pdf, buf, NULL, 0);
		pdf_array_push_drop(ctx, list, added);
	}
	fz_always(ctx)
	{
		fz_drop_buffer(ctx, head);
		fz_drop_buffer(ctx, mark);
	}
	fz_catch(ctx)
		fz_rethrow(ctx);
	return added;
}

/* Text laid out: the pen, code points, lines, placement. */
typedef struct
{
	pen p;
	int *u, n;
	int *from, *to, lines;
	float widest;
	fz_point first;
	float gap;
} laid;

static void drop_laid(fz_context *ctx, laid *l)
{
	fz_drop_font(ctx, l->p.font);
	pdf_drop_obj(ctx, l->p.ref);
	fz_free(ctx, l->u);
	fz_free(ctx, l->from);
	fz_free(ctx, l->to);
}

/* Lays `a` out from `first`, or names a missing glyph. */
static int lay_text(fz_context *ctx, text_args *a, fz_font *own, fz_point first, float gap, laid *l)
{
	int i;
	choose_pen(ctx, a->font, own, &l->p);
	l->n = runes(ctx, a->text, &l->u);
	for (i = 0; i < l->n; i++)
		if (l->u[i] != '\n' && !glyph(ctx, &l->p, l->u[i]))
			return l->u[i];
	l->from = fz_malloc_array(ctx, l->n + 2, int);
	l->to = fz_malloc_array(ctx, l->n + 2, int);
	l->lines = lay_out(ctx, &l->p, l->u, l->n, a->size, a->box[2], l->from, l->to, &l->widest);
	l->first = first;
	l->gap = gap;
	return 0;
}

/* The area laid text covers, in page space. */
static fz_rect laid_area(laid *l, float size)
{
	return fz_make_rect(l->first.x, l->first.y - size, l->first.x + l->widest, l->first.y + (l->lines - 1) * l->gap + size * 0.3f);
}

/* Writes laid text onto `page` of `pdf`. */
static void draw_laid(fz_context *ctx, pdf_document *pdf, pdf_page *page, laid *l, text_args *a)
{
	fz_buffer *buf = NULL;
	fz_var(buf);
	fz_try(ctx)
	{
		fz_matrix to_user = ely_to_user(ctx, page);
		char name[32];
		int i, line;
		pdf_drop_obj(ctx, l->p.ref);
		l->p.ref = NULL;
		add_pen(ctx, pdf, &l->p);
		ely_add_resource(ctx, pdf, page->obj, PDF_NAME(Font), "ElyF", l->p.ref, name, sizeof name);
		buf = fz_new_buffer(ctx, 256);
		fz_append_string(ctx, buf, "q\n");
		for (line = 0; line < l->lines; line++)
		{
			fz_matrix tm = fz_concat(fz_make_matrix(1, 0, 0, -1, l->first.x, l->first.y + line * l->gap), to_user);
			fz_append_printf(ctx, buf, "BT /%s %g Tf %g %g %g rg %g %g %g %g %g %g Tm <", name, a->size,
				((a->rgb >> 16) & 255) / 255.0f, ((a->rgb >> 8) & 255) / 255.0f, (a->rgb & 255) / 255.0f,
				tm.a, tm.b, tm.c, tm.d, tm.e, tm.f);
			for (i = l->from[line]; i < l->to[line]; i++)
				code(ctx, buf, &l->p, l->u[i], glyph(ctx, &l->p, l->u[i]));
			fz_append_string(ctx, buf, "> Tj ET\n");
		}
		fz_append_string(ctx, buf, "Q\n");
		ely_append_content(ctx, pdf, page->obj, buf);
	}
	fz_always(ctx)
		fz_drop_buffer(ctx, buf);
	fz_catch(ctx)
		fz_rethrow(ctx);
}

/* Edits a snapshot copy and draws `region`; the document stays. */
static void preview(fz_context *ctx, pdf_document *pdf, int number, int block, const char *expected, text_args *a,
	laid *l, const char *password, fz_rect region, float scale, unsigned char **pixels, int *w, int *h)
{
	fz_buffer *buf = fz_new_buffer(ctx, 4096);
	fz_output *out = NULL;
	fz_document *copy = NULL;
	pdf_page *page = NULL;
	ely_shape shape = { NULL };
	fz_var(out);
	fz_var(copy);
	fz_var(page);
	fz_try(ctx)
	{
		int tile[4];
		out = fz_new_output_with_buffer(ctx, buf);
		pdf_write_snapshot(ctx, pdf, out);
		fz_close_output(ctx, out);
		copy = fz_open_document_with_buffer(ctx, "application/pdf", buf);
		if (fz_needs_password(ctx, copy) && !fz_authenticate_password(ctx, copy, password))
			fz_throw(ctx, FZ_ERROR_ARGUMENT, "the copy refused its password");
		page = pdf_load_page(ctx, ely_pdf_of(ctx, copy), number);
		if (block >= 0)
		{
			ely_read_block(ctx, copy, page, number, block, expected, &shape);
			ely_remove_block(ctx, ely_pdf_of(ctx, copy), page, &shape);
		}
		draw_laid(ctx, ely_pdf_of(ctx, copy), page, l, a);
		region = fz_expand_rect(region, 8);
		tile[0] = (int)floorf(region.x0 * scale), tile[1] = (int)floorf(region.y0 * scale);
		tile[2] = (int)ceilf(region.x1 * scale) - tile[0], tile[3] = (int)ceilf(region.y1 * scale) - tile[1];
		*pixels = fz_malloc(ctx, (size_t)tile[2] * tile[3] * 4);
		if (ely_render(ctx, copy, number, scale, 0, tile, 0, *pixels) < 0)
			fz_throw(ctx, FZ_ERROR_GENERIC, "%s", ely_error());
		*w = tile[2], *h = tile[3];
	}
	fz_always(ctx)
	{
		ely_drop_shape(ctx, &shape);
		fz_drop_page(ctx, (fz_page *)page);
		fz_drop_document(ctx, copy);
		fz_drop_output(ctx, out);
		fz_drop_buffer(ctx, buf);
	}
	fz_catch(ctx)
		fz_rethrow(ctx);
}

/* Edits, adds or previews text: 0, 2 missing, -1 failed. */
int ely_put_text(fz_context *ctx, fz_document *doc, int number, int block, const char *expected, const char *text,
	const char *font, float size, int rgb, const float box[3], const char *password, float scale, int *missing,
	int *grew, unsigned char **pixels, int *w, int *h)
{
	pdf_document *pdf = NULL;
	pdf_page *page = NULL;
	ely_shape shape = { NULL };
	laid l = { { PEN_SIMPLE, NULL, 0, NULL } };
	int begun = 0;
	fz_var(pdf);
	fz_var(page);
	fz_var(begun);
	*pixels = NULL;
	*grew = 0;
	fz_try(ctx)
	{
		text_args a = { text, font, size, rgb, { box[0], box[1], box[2] } };
		fz_point first = fz_make_point(box[0], box[1] + size * 0.8f);
		float gap = size * 1.2f;
		pdf = ely_pdf_of(ctx, doc);
		page = pdf_load_page(ctx, pdf, number);
		/* Checks precede the step, so redo survives a refusal. */
		if (block >= 0)
		{
			ely_read_block(ctx, doc, page, number, block, expected, &shape);
			first = shape.first;
			gap = shape.gap;
		}
		*missing = lay_text(ctx, &a, shape.font, first, gap, &l);
		if (!*missing)
		{
			fz_rect drawn = laid_area(&l, size), region = drawn;
			if (block >= 0)
			{
				fz_rect room = fz_expand_rect(shape.bbox, 1);
				region = fz_union_rect(shape.bbox, drawn);
				*grew = fabsf(size - shape.size) > 0.01f || !fz_contains_rect(room, drawn);
			}
			if (scale > 0)
				preview(ctx, pdf, number, block, expected, &a, &l, password, region, scale, pixels, w, h);
			else
			{
				pdf_begin_operation(ctx, pdf, block >= 0 ? "Edit text" : "Add text");
				begun = 1;
				if (block >= 0)
					ely_remove_block(ctx, pdf, page, &shape);
				draw_laid(ctx, pdf, page, &l, &a);
				pdf_end_operation(ctx, pdf);
				begun = 0;
			}
		}
	}
	fz_always(ctx)
	{
		drop_laid(ctx, &l);
		ely_drop_shape(ctx, &shape);
		fz_drop_page(ctx, (fz_page *)page);
	}
	fz_catch(ctx)
	{
		fz_free(ctx, *pixels);
		*pixels = NULL;
		return ely_edit_failed(ctx, pdf, begun);
	}
	return *missing ? 2 : 0;
}
