from django.db import migrations


class Migration(migrations.Migration):
    """Join the latest app migration line with the Nuvio synchronization line."""

    dependencies = [
        ("app", "0160_item_imdb_rating_item_imdb_rating_count"),
        ("app", "0164_watchedstate_snapshot_index"),
    ]

    operations = []
