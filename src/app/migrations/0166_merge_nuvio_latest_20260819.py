from django.db import migrations


class Migration(migrations.Migration):
    """Join the current app migration leaf with the Nuvio synchronization line."""

    dependencies = [
        ("app", "0161_merge_20260818_2329"),
        ("app", "0165_merge_20260818_2055"),
    ]

    operations = []
