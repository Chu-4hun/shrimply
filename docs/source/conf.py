# Configuration file for the Sphinx documentation builder.

project = "Shrimply Documentation"
copyright = "Shrimply contributors"
author = "Shrimply contributors"

extensions = [
    "sphinx_tabs.tabs",
    "sphinxext.opengraph",
]
source_suffix = {".rst": "restructuredtext"}
templates_path = ["_templates"]
exclude_patterns = []

html_title = "Shrimply Documentation"
html_theme = "furo"
html_theme_options = {
    "light_css_variables": {
        "color-brand-primary": "#4a86cf",
        "color-brand-content": "#4a86cf",
    },
    "source_edit_link": "https://github.com/soirihiroka/shrimply/edit/main/docs/source/{filename}",
}
html_favicon = "_static/shrimply.svg"
html_static_path = ["_static"]
html_css_files = ["gnome.css"]
html_show_copyright = 0
html_show_sphinx = 0
show_source = 0

ogp_image = "_static/shrimply.svg"
