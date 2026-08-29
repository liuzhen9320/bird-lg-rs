(function () {
	"use strict";

	const form = document.getElementById("goto-form");
	const birdActions = new Set([
		"detail",
		"route_from_protocol",
		"route_from_protocol_all",
		"route_from_protocol_primary",
		"route_from_protocol_all_primary",
		"route_filtered_from_protocol",
		"route_filtered_from_protocol_all",
		"route_from_origin",
		"route_from_origin_all",
		"route_from_origin_primary",
		"route_from_origin_all_primary",
		"route",
		"route_all",
		"route_where",
		"route_where_all",
		"route_generic",
		"generic",
		"route_bgpmap",
		"route_where_bgpmap",
	]);
	const allowedActions = new Set([
		"summary",
		"whois",
		"traceroute",
		...birdActions,
	]);

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

		if (action === "summary") {
			url = "/summary/" + encodedServer + "/";
		} else if (action === "whois") {
			url = "/whois/" + encodedTarget;
		} else if (action === "traceroute") {
			url = "/traceroute/" + encodedServer + "/" + encodedTarget;
		} else if (birdActions.has(action)) {
			url = "/" + action + "/" + encodedServer + "/" + encodedTarget;
		} else {
			return;
		}

		window.location.href = url;
	});
})();
