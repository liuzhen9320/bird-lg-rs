(function () {
	"use strict";

	const form = document.getElementById("goto-form");
	const allowedActions = new Set(["whois", "summary", "ping", "dns"]);

	form.addEventListener("submit", function (event) {
		event.preventDefault();

		const action = form.elements.action.value;
		const server = form.elements.server.value;
		const target = form.elements.target.value;

		if (!allowedActions.has(action)) {
			return;
		}

		const encodedServer = encodeURIComponent(server || "");
		const encodedTarget = encodeURIComponent(target || "");
		let url;

		if (action === "whois") {
			url = "/" + action + "/" + encodedTarget;
		} else if (action === "summary") {
			url = "/" + action + "/" + encodedServer + "/";
		} else {
			url = "/" + action + "/" + encodedServer + "/" + encodedTarget;
		}

		window.location.href = url;
	});
})();
