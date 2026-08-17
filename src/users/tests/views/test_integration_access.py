from bs4 import BeautifulSoup
from django.contrib.auth import get_user_model
from django.test import TestCase
from django.urls import reverse

from integrations.models import IntegrationToken


class IntegrationAccessViewTests(TestCase):
    """Verify the scoped integration access settings surface."""

    def setUp(self):
        self.user = get_user_model().objects.create_user(
            username="integration-access-user",
            password="testpass123",
        )

    def test_signed_out_user_is_redirected(self):
        response = self.client.get(reverse("integration_access"))

        self.assertEqual(response.status_code, 302)
        self.assertIn("/accounts/login", response.url)

    def test_page_has_persistent_accessible_status_and_secret_regions(self):
        self.client.force_login(self.user)
        response = self.client.get(reverse("integration_access"))

        self.assertEqual(response.status_code, 200)
        self.assertTemplateUsed(response, "users/integration_access.html")
        soup = BeautifulSoup(response.content, "html.parser")

        current_link = soup.find("a", attrs={"aria-current": "page"})
        self.assertIsNotNone(current_link)
        self.assertEqual(current_link.get_text(" ", strip=True), "Integration Access")

        status = soup.find(id="integration-token-status")
        error = soup.find(id="integration-token-error")
        secret_panel = soup.find(id="integration-token-secret-panel")
        secret_input = soup.find(id="integration-token-secret")

        self.assertEqual(status.get("role"), "status")
        self.assertEqual(status.get("aria-live"), "polite")
        self.assertEqual(error.get("role"), "alert")
        self.assertIsNotNone(secret_panel.get("hidden"))
        self.assertTrue(secret_input.has_attr("readonly"))
        self.assertEqual(secret_input.get("autocomplete"), "off")

    def test_existing_token_secret_is_not_embedded_in_page(self):
        _token, raw_token = IntegrationToken.generate(
            user=self.user,
            name="Existing scoped token",
            scopes=["progress:read"],
        )
        self.client.force_login(self.user)

        response = self.client.get(reverse("integration_access"))

        self.assertEqual(response.status_code, 200)
        self.assertNotContains(response, raw_token)
        self.assertNotContains(response, "token_digest")
