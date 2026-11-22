/*
 * ranote Android smoke test.
 *
 * Proves the pinned GTK + libadwaita Android toolchain: a window with an
 * Adwaita toolbar, a status page and one interactive button. No rnote code.
 *
 * Entry point requirements from GTK's Android builder:
 *   - main() ends in g_application_run()
 *   - meson target uses android_exe_type: 'application'
 */

#include <adwaita.h>
#include <gtk/gtk.h>

static void
on_button_clicked (GtkButton *button, gpointer user_data)
{
  adw_status_page_set_description (ADW_STATUS_PAGE (user_data),
                                   "Input reached the app. GL renderer is up.");
  gtk_widget_set_sensitive (GTK_WIDGET (button), FALSE);
}

static void
on_activate (GtkApplication *app, gpointer user_data)
{
  (void) user_data;
  GtkWidget *window = adw_application_window_new (app);
  GtkWidget *toolbar_view = adw_toolbar_view_new ();
  GtkWidget *header_bar = adw_header_bar_new ();
  GtkWidget *status_page = adw_status_page_new ();
  GtkWidget *button = gtk_button_new_with_label ("Tap to test input");
  gchar *gtk_version = g_strdup_printf ("GTK %d.%d.%d",
                                        gtk_get_major_version (),
                                        gtk_get_minor_version (),
                                        gtk_get_micro_version ());
  gchar *adw_version = g_strdup_printf ("libadwaita %d.%d.%d",
                                        adw_get_major_version (),
                                        adw_get_minor_version (),
                                        adw_get_micro_version ());
  const gchar *gsk_renderer = g_getenv ("GSK_RENDERER");
  gchar *description = g_strdup_printf ("%s · %s\nGSK_RENDERER=%s",
                                        gtk_version,
                                        adw_version,
                                        gsk_renderer ? gsk_renderer : "unset");

  gtk_window_set_title (GTK_WINDOW (window), "ranote Android smoke");
  gtk_window_set_default_size (GTK_WINDOW (window), 800, 600);

  adw_header_bar_set_title_widget (ADW_HEADER_BAR (header_bar),
                                   gtk_label_new ("ranote Android smoke"));

  adw_status_page_set_title (ADW_STATUS_PAGE (status_page),
                             "GTK + libadwaita on Android");
  adw_status_page_set_description (ADW_STATUS_PAGE (status_page), description);
  gtk_widget_set_margin_start (button, 24);
  gtk_widget_set_margin_end (button, 24);
  gtk_widget_set_halign (button, GTK_ALIGN_CENTER);
  adw_status_page_set_child (ADW_STATUS_PAGE (status_page), button);

  adw_toolbar_view_add_top_bar (ADW_TOOLBAR_VIEW (toolbar_view), header_bar);
  adw_toolbar_view_set_content (ADW_TOOLBAR_VIEW (toolbar_view), status_page);
  adw_application_window_set_content (ADW_APPLICATION_WINDOW (window),
                                      toolbar_view);

  g_signal_connect (button, "clicked", G_CALLBACK (on_button_clicked), status_page);

  gtk_window_present (GTK_WINDOW (window));

  {
    GskRenderer *renderer =
        gtk_native_get_renderer (GTK_NATIVE (gtk_widget_get_native (window)));
    g_message ("%s, display=%s, renderer=%s",
               description,
               gdk_display_get_name (gdk_display_get_default ()),
               renderer ? G_OBJECT_TYPE_NAME (renderer) : "not-realized");
  }

  g_free (gtk_version);
  g_free (adw_version);
  g_free (description);
}

int
main (int argc, char **argv)
{
  AdwApplication *app;

  /*
   * The spike gates on GL: if GL cannot initialize, fail with logs instead of
   * silently falling back to another renderer.
   */
  g_setenv ("GSK_RENDERER", "gl", TRUE);

  app = adw_application_new ("org.ranote.Smoke", G_APPLICATION_DEFAULT_FLAGS);
  g_signal_connect (app, "activate", G_CALLBACK (on_activate), NULL);

  return g_application_run (G_APPLICATION (app), argc, argv);
}
