$(function () {
    let $host = $('#host');
    let $spinner = $('#spinner');
    let $fast_hand = $('#fast_hand');
    let $slow_hand = $('#slow_hand');
    let $link_stat = $('#link_stat');
    let $pkt_num = $('#pkt_num');
    let $inbound = $('#inbound');
    let $outbound = $('#outbound');
    let $raw_data = $('#raw_data');
    let $play_pause = $('#play_pause');

    var ait = null;  // outbound AIT
    let get_ait = function (s) {
        let i = s.indexOf('\u0000');
        if (i < 0) {
            i = s.length;
        }
        return s.slice(0, i);
    }

    var waiting = false;  // waiting for server response
    let refresh = function () {
        if (waiting) {
            $raw_data.addClass('error');
            return;  // prevent overlapping requests
        }
        waiting = true;
        $raw_data.removeClass('error');
        var params = {};
        if (typeof ait === 'string') {
            params.ait = ait;  // FIXME: only send 64 bits at a time?
        }
        $.getJSON('/ebpf_map/ait.json', params)
            .done(update);
    };
    let update = function (data) {
        if (data.ait_map[1].n !== -1) {
            // inbound AIT
            var s = get_ait(data.ait_map[1].s);
            //$inbound.text($inbound.text() + s);
            $inbound.append(document.createTextNode(s));
        }
        if (typeof data.sent === 'string') {
            if (ait.startsWith(data.sent)) {
                ait = ait.slice(data.sent.length);
            } else {
                console.log('WARNING! '
                    + 'expected "' + data.sent + '"'
                    + 'as prefix of "' + ait + '"');
            }
            if (ait.length == 0) {
                ait = null;
            }
        }
        if (typeof data.link === 'string') {
            if (data.link == 'INIT') {
                $link_stat.css({ "color": "#333",
                      "background-color": "#FF0" });
            } else if (data.link == 'UP') {
                $link_stat.css({ "color": "#FFF",
                      "background-color": "#0C0" });
            } else if (data.link == 'DOWN') {
                $link_stat.css({ "color": "#000",
                      "background-color": "#F00" });
            } else if (data.link == 'DEAD') {
                $link_stat.css({ "color": "#999",
                      "background-color": "#000" });
            } else {
                $link_stat.css({ "color": "#666",
                      "background-color": "#CCC" });
            }
            $link_stat.text(data.link);
        }
        if (typeof data.host === 'string') {
            $host.text(' ('+data.host+')');
        }
        var cnt = data.ait_map[3];
        $pkt_num.val(cnt.n);
        var fast_rot = (((cnt.b[1] << 8) | cnt.b[0]) * 360) >> 16;
        $fast_hand.attr('transform', 'rotate(' + fast_rot + ')');
        var slow_rot = (cnt.b[2] * 360) >> 8;
        $slow_hand.attr('transform', 'rotate(' + slow_rot + ')');
        $raw_data.text(JSON.stringify(data, null, 2));
        waiting = false;
    };

    var animation;
    let animate = function (timestamp) {
        refresh();
        animation = requestAnimationFrame(animate);
    };
    let toggleRefresh = function () {
        if (animation) {
            cancelAnimationFrame(animation);
            animation = undefined;
            $('#pause').hide();
            $('#play').show();
        } else {
            $('#play').hide();
            $('#pause').show();
            animate();
        }
    };

    $play_pause.click(function (e) {
        toggleRefresh();
    });

    $('#send').click(function (e) {
        ait = $outbound.val() + '\n';
        $outbound.val('');
    });

    $('#debug').click(function (e) {
        $raw_data.toggleClass('hidden');
    });

    animate();
});
